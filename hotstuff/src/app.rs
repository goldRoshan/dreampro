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

    // For a full HotStuff integration, add sync-specific validation here if required.
}
