<<<<<<< HEAD
use std::sync::{Arc, Mutex};

use hotstuff_rs::app::{App, ProduceBlockRequest, ProduceBlockResponse, ValidateBlockRequest, ValidateBlockResponse};
use hotstuff_rs::block_tree::pluggables::KVStore as HsKVStore;
use hotstuff_rs::types::{crypto_primitives::{CryptoHasher, Digest}, data_types::{CryptoHash, Data, Datum}};
use hotstuff_rs::types::update_sets::{AppStateUpdates, ValidatorSetUpdates};
use hotstuff_rs::types::validator_set::VerifyingKey;
use hotstuff_rs::types::data_types::Power;
use base64::Engine;
use std::collections::{BTreeMap, HashSet};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Order payload submitted by users and gossiped between nodes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub symbol: String,
    pub price: f64,
    pub side: String,
    pub position: u64,
}

/// Minimal order-book App: collects orders and commits them in blocks.
#[derive(Clone, Default)]
pub struct CounterApp {
    pub orders: Arc<Mutex<Vec<Order>>>,
    pub vs_inserts: Arc<Mutex<Vec<(VerifyingKey, Power)>>>,
    pub vs_deletes: Arc<Mutex<Vec<VerifyingKey>>>,
    pub seen_ids: Arc<Mutex<HashSet<Uuid>>>,
}

impl CounterApp { pub fn new() -> Self { Self { orders: Arc::new(Mutex::new(Vec::new())), vs_inserts: Arc::new(Mutex::new(Vec::new())), vs_deletes: Arc::new(Mutex::new(Vec::new())), seen_ids: Arc::new(Mutex::new(HashSet::new())) } } }

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BlockPayloadSerde {
    orders: Vec<Order>,
    vs_inserts: Vec<(String, u64)>,
    vs_deletes: Vec<String>,
}

fn key_buy(symbol: &str) -> Vec<u8> { format!("book:{}:buy", symbol).into_bytes() }
fn key_sell(symbol: &str) -> Vec<u8> { format!("book:{}:sell", symbol).into_bytes() }

fn sort_orders_by_price(orders: &mut Vec<Order>, side: &str) {
    if side.eq_ignore_ascii_case("buy") {
        orders.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap());
    } else {
        orders.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
    }
}

impl<K: HsKVStore> App<K> for CounterApp {
    fn produce_block(&mut self, request: ProduceBlockRequest<K>) -> ProduceBlockResponse {
        // Drain pending orders and VS updates; sort deterministically for stability.
        let mut drained = {
            let mut guard = self.orders.lock().unwrap();
            let v = guard.drain(..).collect::<Vec<_>>();
            v
        };
        let drained_len = drained.len();
        drained.sort_by(|a, b| serde_json::to_vec(a).unwrap().cmp(&serde_json::to_vec(b).unwrap()));
        let drained_inserts = { let mut g = self.vs_inserts.lock().unwrap(); g.drain(..).collect::<Vec<_>>() };
        let drained_deletes = { let mut g = self.vs_deletes.lock().unwrap(); g.drain(..).collect::<Vec<_>>() };
        if drained_len > 0 || !drained_inserts.is_empty() || !drained_deletes.is_empty() {
            log::info!(
                "App.produce_block: drained_orders={} vs_inserts={} vs_deletes={}",
                drained_len,
                drained_inserts.len(),
                drained_deletes.len()
            );
        } else {
            log::debug!("App.produce_block: drained_orders=0 (empty view)");
        }
        let vs_inserts_ser: Vec<(String, u64)> = drained_inserts.iter().map(|(vk,p)| (base64::engine::general_purpose::STANDARD_NO_PAD.encode(vk.to_bytes()), p.int())).collect();
        let vs_deletes_ser: Vec<String> = drained_deletes.iter().map(|vk| base64::engine::general_purpose::STANDARD_NO_PAD.encode(vk.to_bytes())).collect();
        let payload_obj = BlockPayloadSerde { orders: drained.clone(), vs_inserts: vs_inserts_ser, vs_deletes: vs_deletes_ser };
        let payload = serde_json::to_vec(&payload_obj).unwrap();
        let data = Data::new(vec![Datum::new(payload)]);
        let mut hasher = CryptoHasher::new();
        let data_ser = borsh::to_vec(&data).unwrap();
        hasher.update(&data_ser);
        let dh: [u8; 32] = hasher.finalize().into();
        let data_hash = CryptoHash::new(dh);
        log::debug!(
            "App.produce_block: data_ser_len={} data_hash_b64={}",
            data_ser.len(),
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(dh)
        );
        let mut vsu = ValidatorSetUpdates::new();
        for (vk, p) in drained_inserts { vsu.insert(vk, p); }
        for vk in drained_deletes { vsu.delete(vk); }
        // Build app state updates: merge drained orders into existing committed order book state keys
        let mut app_updates = AppStateUpdates::new();
        if !drained.is_empty() {
            // Group by (symbol, side)
            let mut groups: BTreeMap<(String, String), Vec<Order>> = BTreeMap::new();
            for o in drained.into_iter() {
                groups.entry((o.symbol.clone(), o.side.clone())).or_default().push(o);
            }
            // Merge with committed state from block tree snapshot available in produce
            for ((symbol, side), mut new_orders) in groups.into_iter() {
                let key = if side.eq_ignore_ascii_case("buy") { key_buy(&symbol) } else { key_sell(&symbol) };
                let mut combined: Vec<Order> = if let Some(bytes) = request.block_tree().app_state(&key) {
                    serde_json::from_slice::<Vec<Order>>(&bytes).unwrap_or_default()
                } else { Vec::new() };
                combined.append(&mut new_orders);
                sort_orders_by_price(&mut combined, &side);
                app_updates.insert(key, serde_json::to_vec(&combined).unwrap());
            }
        }
        ProduceBlockResponse { data_hash, data, app_state_updates: Some(app_updates), validator_set_updates: Some(vsu) }
    }

    fn validate_block(&mut self, request: ValidateBlockRequest<K>) -> ValidateBlockResponse {
        // Accept if data hash matches deterministic hash of payload.
        log::debug!("App.validate_block: called");
        self.validate_block_for_sync(request)
    }

    fn validate_block_for_sync(&mut self, request: ValidateBlockRequest<K>) -> ValidateBlockResponse {
        let data = request.proposed_block().data.clone();
        // If no data is attached (hash-only propagation), accept by hash; updates were included at produce time.
        if data.vec().is_empty() {
            log::debug!("App.validate_block_for_sync: no data attached; accepting by hash");
            return ValidateBlockResponse::Valid { app_state_updates: None, validator_set_updates: None };
        }
        let mut hasher = CryptoHasher::new();
        let data_ser = borsh::to_vec(&data).unwrap();
        hasher.update(&data_ser);
        let dh: [u8; 32] = hasher.finalize().into();
        let header_hash = request.proposed_block().data_hash;
        let comp_hash = CryptoHash::new(dh);
        log::info!(
            "App.validate_block_for_sync: data_ser_len={} header_hash={} computed_hash={}",
            data_ser.len(),
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(header_hash.bytes()),
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(comp_hash.bytes())
        );
        if header_hash == comp_hash {
            log::info!("App.validate_block_for_sync: valid payload (data_ser_len={})", data_ser.len());
            // Reconstruct validator set updates and app state updates from payload
            let mut vsu = ValidatorSetUpdates::new();
            let mut asu = AppStateUpdates::new();
            if let Some(datum) = data.vec().get(0) {
            if let Ok(obj) = serde_json::from_slice::<BlockPayloadSerde>(datum.bytes()) {
                // VS updates
                for (vk_b64, p) in obj.vs_inserts {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(vk_b64) {
                        if let Ok(vk) = VerifyingKey::from_bytes(&bytes[..].try_into().unwrap_or([0u8;32])) {
                            vsu.insert(vk, Power::new(p));
                        }
                    }
                }
                for vk_b64 in obj.vs_deletes {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(vk_b64) {
                        if let Ok(vk) = VerifyingKey::from_bytes(&bytes[..].try_into().unwrap_or([0u8;32])) {
                            vsu.delete(vk);
                        }
                    }
                }
                // Order book updates: group and merge with committed state
                let mut groups: BTreeMap<(String, String), Vec<Order>> = BTreeMap::new();
                for o in obj.orders.into_iter() {
                    groups.entry((o.symbol.clone(), o.side.clone())).or_default().push(o);
                }
                for ((symbol, side), mut new_orders) in groups.into_iter() {
                    let key = if side.eq_ignore_ascii_case("buy") { key_buy(&symbol) } else { key_sell(&symbol) };
                    let mut combined: Vec<Order> = if let Some(bytes) = request.block_tree().app_state(&key) {
                        serde_json::from_slice::<Vec<Order>>(&bytes).unwrap_or_default()
                    } else { Vec::new() };
                    combined.append(&mut new_orders);
                    sort_orders_by_price(&mut combined, &side);
                    asu.insert(key, serde_json::to_vec(&combined).unwrap());
                }
            }}
            ValidateBlockResponse::Valid { app_state_updates: Some(asu), validator_set_updates: Some(vsu) }
        } else {
            log::warn!("App.validate_block_for_sync: INVALID payload hash");
            ValidateBlockResponse::Invalid
        }
    }
}

impl CounterApp {
    pub fn submit_order(&self, o: Order) {
        let mut seen = self.seen_ids.lock().unwrap();
        if seen.insert(o.id) {
            drop(seen);
            let mut g = self.orders.lock().unwrap();
            g.push(o);
        } else {
            log::debug!("App.submit_order: duplicate id {} ignored", o.id);
        }
    }

    pub fn add_validator(&self, vk: VerifyingKey, power: Power) {
        self.vs_inserts.lock().unwrap().push((vk, power));
    }
    #[allow(dead_code)]
    pub fn remove_validator(&self, vk: VerifyingKey) {
        self.vs_deletes.lock().unwrap().push(vk);
    }
}
=======
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Minimal deterministic app used by the demo.
#[derive(Clone)]
pub struct CounterApp {
    counter: Arc<AtomicU64>,
}

impl CounterApp {
    pub fn new() -> Self {
        Self { counter: Arc::new(AtomicU64::new(0)) }
    }

    /// Produce a small, deterministic payload.
    pub fn produce_block(&self) -> Vec<u8> {
        // Deterministic increment; monotonic per-process.
        let v = self.counter.fetch_add(1, Ordering::Relaxed);
        format!("tick-{v}").into_bytes()
    }

    /// Validate a block payload (accepts all for demo).
    pub fn validate_block(&self, _data: &[u8]) -> bool { true }

    /// Validate block for sync (same as validate for this demo).
    pub fn validate_block_for_sync(&self, data: &[u8]) -> bool { self.validate_block(data) }
}

>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)
