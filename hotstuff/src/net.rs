use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver, unbounded_channel};
use tokio::sync::Mutex;

/// A simple in-proc network message.
#[derive(Clone, Debug)]
pub struct Message {
    pub from: String,
    pub kind: MsgKind,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug)]
pub enum MsgKind {
    Proposal(u64),
    Vote(u64),
    Commit(u64),
}

/// In-process network: full-mesh map of `peer_id` -> sender.
#[derive(Clone)]
pub struct InProcNet {
    #[allow(dead_code)]
    me: String,
    rx: Arc<Mutex<UnboundedReceiver<Message>>>,
    peers: Arc<Mutex<BTreeMap<String, UnboundedSender<Message>>>>,
}

impl InProcNet {
    /// Create a node's network with its own mailbox; returns (net, my_sender).
    pub fn new(me: String) -> (Self, UnboundedSender<Message>) {
        let (tx, rx) = unbounded_channel();
        (
            Self {
                me,
                rx: Arc::new(Mutex::new(rx)),
                peers: Arc::new(Mutex::new(BTreeMap::new())),
            },
            tx,
        )
    }

    /// Connect a remote peer id to its mailbox tx.
    pub async fn connect(&self, peer_id: String, tx: UnboundedSender<Message>) {
        self.peers.lock().await.insert(peer_id, tx);
    }

    /// Broadcast a message to all peers (including self if present).
    pub async fn broadcast(&self, mut msg: Message) {
        let peers = self.peers.lock().await.clone();
        for (pid, tx) in peers.into_iter() {
            let _ = tx.send(Message { from: msg.from.clone(), ..msg.clone() });
            msg.from = pid.clone(); // keep type happy; not strictly needed
        }
    }

    /// Send a message to a specific peer.
    pub async fn send(&self, peer: &str, msg: Message) {
        if let Some(tx) = self.peers.lock().await.get(peer).cloned() {
            let _ = tx.send(msg);
        }
    }

    /// Non-blocking receive. Returns None if no message is available.
    pub async fn recv(&self) -> Option<Message> {
        let mut rx = self.rx.lock().await;
        rx.try_recv().ok()
    }

    pub fn me(&self) -> &str { &self.me }
}
