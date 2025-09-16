use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use hotstuff_rs::block_tree::pluggables::{KVGet, KVStore as HsKVStore, WriteBatch as HsWriteBatch};

/// In-memory KV store implementing HotStuff's KVStore trait.
#[derive(Clone, Default)]
pub struct KVStore(Arc<RwLock<BTreeMap<Vec<u8>, Vec<u8>>>>);

impl KVStore {
    pub fn new() -> Self { Self::default() }
}

impl KVGet for KVStore {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> { self.0.read().unwrap().get(key).cloned() }
}

impl HsKVStore for KVStore {
    type WriteBatch = WriteBatch;
    type Snapshot<'a> = Snapshot where Self: 'a;

    fn write(&mut self, wb: Self::WriteBatch) {
        let mut guard = self.0.write().unwrap();
        for op in wb.ops {
            match op {
                Op::Set(k, v) => { guard.insert(k, v); }
                Op::Del(k) => { guard.remove(&k); }
            }
        }
    }

    fn clear(&mut self) { self.0.write().unwrap().clear(); }

    fn snapshot<'b>(&'b self) -> Self::Snapshot<'b> { Snapshot(self.0.read().unwrap().clone()) }
}

#[derive(Clone)]
pub struct Snapshot(BTreeMap<Vec<u8>, Vec<u8>>);

impl KVGet for Snapshot {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> { self.0.get(key).cloned() }
}

#[derive(Clone, Default)]
pub struct WriteBatch {
    ops: Vec<Op>,
}

#[derive(Clone)]
enum Op { Set(Vec<u8>, Vec<u8>), Del(Vec<u8>), }

impl WriteBatch {
    pub fn put<K: Into<Vec<u8>>, V: Into<Vec<u8>>>(&mut self, k: K, v: V) { self.ops.push(Op::Set(k.into(), v.into())); }
    pub fn delete<K: Into<Vec<u8>>>(&mut self, k: K) {
        self.ops.push(Op::Del(k.into()));
    }
}

impl HsWriteBatch for WriteBatch {
    fn new() -> Self { Self { ops: vec![] } }
    fn set(&mut self, key: &[u8], value: &[u8]) { self.put(key.to_vec(), value.to_vec()); }
    fn delete(&mut self, key: &[u8]) { self.delete(key.to_vec()); }
}
