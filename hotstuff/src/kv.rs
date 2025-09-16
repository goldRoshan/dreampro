use std::collections::BTreeMap;
<<<<<<< HEAD
use std::sync::{Arc, RwLock};

use hotstuff_rs::block_tree::pluggables::{KVGet, KVStore as HsKVStore, WriteBatch as HsWriteBatch};

/// In-memory KV store implementing HotStuff's KVStore trait.
#[derive(Clone, Default)]
pub struct KVStore(Arc<RwLock<BTreeMap<Vec<u8>, Vec<u8>>>>);

impl KVStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl KVGet for KVStore {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.0.read().unwrap().get(key).cloned()
    }
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

    fn clear(&mut self) {
        self.0.write().unwrap().clear();
    }

    fn snapshot<'b>(&'b self) -> Self::Snapshot<'b> { Snapshot(self.0.read().unwrap().clone()) }
}

#[derive(Clone)]
pub struct Snapshot(BTreeMap<Vec<u8>, Vec<u8>>);

impl KVGet for Snapshot {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.0.get(key).cloned()
    }
}

#[derive(Clone, Default)]
=======
use std::sync::{Arc};
use tokio::sync::RwLock;

/// A write batch consisting of put/delete operations applied atomically.
#[derive(Default, Clone)]
>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)
pub struct WriteBatch {
    ops: Vec<Op>,
}

#[derive(Clone)]
enum Op {
<<<<<<< HEAD
    Set(Vec<u8>, Vec<u8>),
=======
    Put(Vec<u8>, Vec<u8>),
>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)
    Del(Vec<u8>),
}

impl WriteBatch {
<<<<<<< HEAD
    pub fn put<K: Into<Vec<u8>>, V: Into<Vec<u8>>>(&mut self, k: K, v: V) {
        self.ops.push(Op::Set(k.into(), v.into()));
=======
    pub fn new() -> Self { Self { ops: Vec::new() } }
    pub fn put<K: Into<Vec<u8>>, V: Into<Vec<u8>>>(&mut self, k: K, v: V) {
        self.ops.push(Op::Put(k.into(), v.into()));
>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)
    }
    pub fn delete<K: Into<Vec<u8>>>(&mut self, k: K) {
        self.ops.push(Op::Del(k.into()));
    }
<<<<<<< HEAD
}

impl HsWriteBatch for WriteBatch {
    fn new() -> Self { Self { ops: vec![] } }
    fn set(&mut self, key: &[u8], value: &[u8]) { self.put(key.to_vec(), value.to_vec()); }
    fn delete(&mut self, key: &[u8]) { self.delete(key.to_vec()); }
}
=======
    pub fn is_empty(&self) -> bool { self.ops.is_empty() }
}

#[derive(Clone, Default)]
pub struct KVStore {
    inner: Arc<RwLock<BTreeMap<Vec<u8>, Vec<u8>>>>,
}

impl KVStore {
    pub fn new() -> Self { Self::default() }

    /// Atomically apply a batch of writes.
    pub async fn write(&self, batch: WriteBatch) {
        if batch.is_empty() { return; }
        let mut map = self.inner.write().await;
        for op in batch.ops.into_iter() {
            match op {
                Op::Put(k, v) => { map.insert(k, v); }
                Op::Del(k) => { map.remove(&k); }
            }
        }
    }

    /// Get a value by key from a read-only snapshot.
    pub async fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let map = self.inner.read().await;
        map.get(key).cloned()
    }

    /// Clear the entire store.
    pub async fn clear(&self) {
        let mut map = self.inner.write().await;
        map.clear();
    }

    /// Create an immutable snapshot for consistent reads.
    pub async fn snapshot(&self) -> Snapshot {
        let map = self.inner.read().await;
        Snapshot { data: map.clone() }
    }
}

#[derive(Clone)]
pub struct Snapshot {
    data: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl Snapshot {
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.data.get(key).cloned()
    }
}

>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)
