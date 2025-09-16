use std::{io, net::SocketAddr, sync::{Arc, Mutex}};
use std::collections::HashMap;

// use borsh::BorshDeserialize; // not needed due to fully-qualified usage
use hotstuff_rs::networking::{messages::Message, network::Network};
use hotstuff_rs::types::{update_sets::ValidatorSetUpdates, validator_set::{ValidatorSet, VerifyingKey}};
use tokio::{net::{TcpListener, TcpStream}, io::{AsyncReadExt, AsyncWriteExt}, sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel}};
use base64::Engine;

/// TCP network implementing the HotStuff Network trait with Borsh serialization.
#[derive(Clone)]
pub struct InProcNet {
    me: VerifyingKey,
    inbox: Arc<Mutex<UnboundedReceiver<(VerifyingKey, Message)>>>,
    /// Loopback to deliver our own outbound messages to the node without going over TCP
    local_tx: UnboundedSender<(VerifyingKey, Message)>,
    peers: Arc<Mutex<Vec<UnboundedSender<(VerifyingKey, Message)>>>>,
    peers_by_vk: Arc<Mutex<HashMap<VerifyingKey, UnboundedSender<(VerifyingKey, Message)>>>>,
}

impl InProcNet {
    pub fn new(me: VerifyingKey, listener: UnboundedReceiver<(VerifyingKey, Message)>, local_tx: UnboundedSender<(VerifyingKey, Message)>) -> Self {
        Self {
            me,
            inbox: Arc::new(Mutex::new(listener)),
            local_tx,
            peers: Arc::new(Mutex::new(Vec::new())),
            peers_by_vk: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // Add an outbound transport before we know the peer's verifying key
    pub async fn add_outbound(&self, tx: UnboundedSender<(VerifyingKey, Message)>) {
        self.peers.lock().unwrap().push(tx);
        let total = self.peers.lock().unwrap().len();
        log::info!("network: added outbound channel (total={})", total);
    }

    #[allow(dead_code)]
    pub fn me(&self) -> VerifyingKey { self.me }

    // Once we learn the peer vk, map it to an existing tx (used for unicast send)
    pub async fn map_peer_vk(&self, vk: VerifyingKey, tx: UnboundedSender<(VerifyingKey, Message)>) {
        self.peers_by_vk.lock().unwrap().insert(vk, tx);
        log::info!("network: mapped peer vk {}", base64::engine::general_purpose::STANDARD_NO_PAD.encode(vk.to_bytes()));
    }
}

impl Network for InProcNet {
    fn init_validator_set(&mut self, _: ValidatorSet) {}
    fn update_validator_set(&mut self, _: ValidatorSetUpdates) {}

    fn broadcast(&mut self, message: Message) {
        // Deliver to self and peers
        let _ = self.local_tx.send((self.me, message.clone()));
        for tx in self.peers.lock().unwrap().iter() { let _ = tx.send((self.me, message.clone())); }
    }

    fn send(&mut self, peer: VerifyingKey, message: Message) {
        if let Some(tx) = self.peers_by_vk.lock().unwrap().get(&peer).cloned() {
            let _ = tx.send((self.me, message));
        } else {
            // fallback: broadcast
            log::debug!("network: unicast vk unknown, broadcasting instead");
            let _ = self.local_tx.send((self.me, message.clone()));
            for tx in self.peers.lock().unwrap().iter() { let _ = tx.send((self.me, message.clone())); }
        }
    }

    fn recv(&mut self) -> Option<(VerifyingKey, Message)> {
        self.inbox.lock().unwrap().try_recv().ok()
    }
}

/// Starts a TCP listener for HotStuff messages on `addr`, delivering deserialized messages into `to_node`.
pub async fn start_network_listener(addr: SocketAddr, to_node: UnboundedSender<(VerifyingKey, Message)>) -> io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tokio::spawn(async move {
        loop {
            if let Ok((mut socket, peer_addr)) = listener.accept().await {
                log::info!("network: accepted connection from {}", peer_addr);
                let to_node = to_node.clone();
                tokio::spawn(async move {
                    // Framing: [32-byte vk][4-byte LE len][len bytes message]
                    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
                    let mut tmp = vec![0u8; 16 * 1024];
                    loop {
                        match socket.read(&mut tmp).await {
                            Ok(0) => break,
                            Ok(n) => {
                                buf.extend_from_slice(&tmp[..n]);
                                // Parse as many complete frames as available
                                loop {
                                    if buf.len() < 32 + 4 { break; }
                                    let mut vk_bytes = [0u8; 32];
                                    vk_bytes.copy_from_slice(&buf[0..32]);
                                    let msg_len = u32::from_le_bytes([
                                        buf[32], buf[33], buf[34], buf[35],
                                    ]) as usize;
                                    let frame_len = 32 + 4 + msg_len;
                                    if buf.len() < frame_len { break; }
                                    // Extract one frame
                                    let origin = match VerifyingKey::from_bytes(&vk_bytes) {
                                        Ok(v) => v,
                                        Err(_) => {
                                            // Drop this frame if vk invalid; resync by skipping one byte
                                            buf.drain(0..1);
                                            continue;
                                        }
                                    };
                                    let msg_bytes = &buf[36..36 + msg_len];
                                    if let Ok(msg) = <Message as borsh::BorshDeserialize>::try_from_slice(msg_bytes) {
                                        let _ = to_node.send((origin, msg));
                                    }
                                    // Remove the frame from buffer
                                    buf.drain(0..frame_len);
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });
            }
        }
    });
    Ok(())
}

/// Connects to a peer and returns a channel; sending to the channel writes to the TCP stream.
pub async fn connect_peer(addr: SocketAddr) -> io::Result<UnboundedSender<(VerifyingKey, Message)>> {
    let mut stream = TcpStream::connect(addr).await?;
    let (tx, mut rx) = unbounded_channel::<(VerifyingKey, Message)>();
    tokio::spawn(async move {
        while let Some((me, msg)) = rx.recv().await {
            let msg_bytes = borsh::to_vec(&msg).unwrap();
            let mut out = Vec::with_capacity(32 + 4 + msg_bytes.len());
            out.extend_from_slice(&me.to_bytes());
            out.extend_from_slice(&(msg_bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(&msg_bytes);
            let _ = stream.write_all(&out).await;
        }
    });
    Ok(tx)
}
