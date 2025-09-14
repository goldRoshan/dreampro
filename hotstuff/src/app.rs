use hotstuff_rs::app::{App, ProduceBlockRequest, ProduceBlockResponse, ValidateBlockRequest, ValidateBlockResponse};
use hotstuff_rs::block_tree::pluggables::KVStore as HsKVStore;
use hotstuff_rs::types::{crypto_primitives::{CryptoHasher, Digest}, data_types::{CryptoHash, Data, Datum}};
use hotstuff_rs::types::update_sets::{AppStateUpdates, ValidatorSetUpdates};

/// Minimal HotStuff App: produces a tiny data payload deterministically and accepts it.
#[derive(Clone, Default)]
pub struct CounterApp;

impl CounterApp { pub fn new() -> Self { Self } }

impl<K: HsKVStore> App<K> for CounterApp {
    fn produce_block(&mut self, _request: ProduceBlockRequest<K>) -> ProduceBlockResponse {
        // Deterministic, tiny payload. We do not read wall-clock or RNG.
        let data = Data::new(vec![Datum::new(b"tick".to_vec())]);
        let mut hasher = CryptoHasher::new();
        hasher.update(&data.vec()[0].bytes());
        let dh: [u8; 32] = hasher.finalize().into();
        let data_hash = CryptoHash::new(dh);
        ProduceBlockResponse { data_hash, data, app_state_updates: Some(AppStateUpdates::new()), validator_set_updates: Some(ValidatorSetUpdates::new()) }
    }

    fn validate_block(&mut self, request: ValidateBlockRequest<K>) -> ValidateBlockResponse {
        // Accept if data hash matches deterministic hash of payload.
        self.validate_block_for_sync(request)
    }

    fn validate_block_for_sync(&mut self, request: ValidateBlockRequest<K>) -> ValidateBlockResponse {
        let data = request.proposed_block().data.clone();
        let mut hasher = CryptoHasher::new();
        hasher.update(&data.vec()[0].bytes());
        let dh: [u8; 32] = hasher.finalize().into();
        if request.proposed_block().data_hash == CryptoHash::new(dh) {
            ValidateBlockResponse::Valid { app_state_updates: Some(AppStateUpdates::new()), validator_set_updates: Some(ValidatorSetUpdates::new()) }
        } else {
            ValidateBlockResponse::Invalid
        }
    }
}
