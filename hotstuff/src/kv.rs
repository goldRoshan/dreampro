use std::collections::BTreeMap;
use std::sync::{Arc};
use tokio::sync::RwLock;

/// A write batch consisting of put/delete operations applied atomically.
#[derive(Default, Clone)]
pub struct WriteBatch {
    ops: Vec<Op>,
}

#[derive(Clone)]
enum Op {
    Put(Vec<u8>, Vec<u8>),
    Del(Vec<u8>),
}

impl WriteBatch {
    pub fn new() -> Self { Self { ops: Vec::new() } }
    pub fn put<K: Into<Vec<u8>>, V: Into<Vec<u8>>>(&mut self, k: K, v: V) {
        self.ops.push(Op::Put(k.into(), v.into()));
    }
    pub fn delete<K: Into<Vec<u8>>>(&mut self, k: K) {
        self.ops.push(Op::Del(k.into()));
    }
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

