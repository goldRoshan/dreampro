use std::collections::HashMap;
use std::sync::{mpsc::{self, Receiver, Sender, TryRecvError}, Arc, Mutex};

use hotstuff_rs::types::crypto_primitives::VerifyingKey;
use hotstuff_rs::networking::{messages::Message, network::Network};
use hotstuff_rs::types::{update_sets::ValidatorSetUpdates, validator_set::ValidatorSet};

/// In-process network implementing the HotStuff Network trait.
#[derive(Clone)]
pub struct InProcNet {
    me: VerifyingKey,
    all_peers: HashMap<VerifyingKey, Sender<(VerifyingKey, Message)>>,
    inbox: Arc<Mutex<Receiver<(VerifyingKey, Message)>>>,
}

impl InProcNet {
    /// Create a full-mesh of in-proc nets given iterator of verifying keys.
    pub fn full_mesh(peers: impl Iterator<Item = VerifyingKey>) -> Vec<InProcNet> {
        let mut all_peers = HashMap::new();
        let mut boxes: Vec<(VerifyingKey, Receiver<(VerifyingKey, Message)>)> = Vec::new();
        let mut keys: Vec<VerifyingKey> = Vec::new();
        for vk in peers {
            let (tx, rx) = mpsc::channel();
            all_peers.insert(vk, tx);
            boxes.push((vk, rx));
            keys.push(vk);
        }
        boxes
            .into_iter()
            .map(|(me, inbox)| InProcNet { me, all_peers: all_peers.clone(), inbox: Arc::new(Mutex::new(inbox)) })
            .collect()
    }

    pub fn me(&self) -> VerifyingKey { self.me }
}

impl Network for InProcNet {
    fn init_validator_set(&mut self, _: ValidatorSet) {}
    fn update_validator_set(&mut self, _: ValidatorSetUpdates) {}

    fn broadcast(&mut self, message: Message) {
        for (_, peer) in &self.all_peers {
            let _ = peer.send((self.me, message.clone()));
        }
    }

    fn send(&mut self, peer: VerifyingKey, message: Message) {
        if let Some(tx) = self.all_peers.get(&peer) {
            let _ = tx.send((self.me, message));
        }
    }

    fn recv(&mut self) -> Option<(VerifyingKey, Message)> {
        match self.inbox.lock().unwrap().try_recv() {
            Ok(t) => Some(t),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => panic!("in-proc net disconnected"),
        }
    }
}
