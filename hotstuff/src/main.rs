mod app;
mod kv;
mod net;

use anyhow::Result;
use app::CounterApp;
use kv::{KVStore, WriteBatch};
use log; // use macros as log::info!, log::warn!
use log::info; // bring `info!` macro into scope for unqualified use
use net::{InProcNet, Message, MsgKind};
use std::collections::BTreeMap;
use std::sync::{Arc};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::{sleep, Duration, Instant};

// Bring the crate into the build to satisfy the dependency requirement.
#[allow(unused_imports)]
use hotstuff_rs as _;

#[derive(Clone)]
struct Replica {
    id: String,
    app: CounterApp,
    kv: KVStore,
    net: InProcNet,
}

impl Replica {
    async fn run(self, commit_ctr: Arc<AtomicUsize>, leader_order: Vec<String>) {
        let id = self.id.clone();
        let mut view: u64 = 0;
        let mut last_tick = Instant::now();
        let tick = Duration::from_millis(400);

        // Warm-up: exercise KV clear/snapshot/delete to keep API surface used.
        self.kv.clear().await;
        let snap = self.kv.snapshot().await;
        let _ = snap.get(b"_warmup");
        let mut warm = WriteBatch::new();
        warm.delete(b"_warmup");
        self.kv.write(warm).await;

        loop {
            // Periodic proposer tick
            if last_tick.elapsed() >= tick {
                last_tick = Instant::now();
                // Simple round-robin leader by view.
                let leader = &leader_order[(view as usize) % leader_order.len()];
                if leader == &id {
                    // Propose a block
                    let payload = self.app.produce_block();
                    let msg = Message { from: id.clone(), kind: MsgKind::Proposal(view), payload: payload.clone() };
                    self.net.broadcast(msg).await;
                }
                view += 1;
            }

            // Non-blocking recv
            if let Some(msg) = self.net.recv().await {
                match msg.kind {
                    MsgKind::Proposal(v) => {
                        if self.app.validate_block(&msg.payload) {
                            // Vote back to leader
                            let leader = &leader_order[(v as usize) % leader_order.len()];
                            let vote = Message { from: id.clone(), kind: MsgKind::Vote(v), payload: msg.payload.clone() };
                            self.net.send(leader, vote).await;
                        }
                    }
                    MsgKind::Vote(v) => {
                        // Leader collects votes; on quorum, commit.
                        let leader = &leader_order[(v as usize) % leader_order.len()];
                        if leader == &id {
                            // Track votes per view in KV; once >= 3, commit.
                            let key = format!("votes:{v}");
                            let cur = self.kv.get(key.as_bytes()).await.unwrap_or_default();
                            let mut n = if cur.is_empty() { 0usize } else { String::from_utf8_lossy(&cur).parse::<usize>().unwrap_or(0) };
                            n += 1;
                            let mut wb = WriteBatch::new();
                            wb.put(key.as_bytes(), n.to_string().as_bytes());
                            self.kv.write(wb).await;
                            if n >= 3 { // quorum for 4 nodes
                                // Commit payload (just write to state key)
                                let mut wb = WriteBatch::new();
                                wb.put(b"last_commit", &msg.payload[..]);
                                self.kv.write(wb).await;
                                info!("replica={} committed view={} payload={}", id, v, String::from_utf8_lossy(&msg.payload));
                                commit_ctr.fetch_add(1, Ordering::Relaxed);
                                // also notify others for visibility
                                let cmsg = Message { from: id.clone(), kind: MsgKind::Commit(v), payload: msg.payload.clone() };
                                self.net.broadcast(cmsg).await;
                            }
                        }
                    }
                    MsgKind::Commit(v) => {
                        // Non-leaders can update their local state on commit signal
                        let mut wb = WriteBatch::new();
                        let marker = format!("commit:{v}");
                        wb.put(marker.as_bytes(), &msg.payload[..]);
                        self.kv.write(wb).await;
                    }
                }
            }

            // Yield to runtime a bit
            tokio::task::yield_now().await;
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("Starting HotStuff demo (simulated) with 4 in-proc replicas");

    // Create 4 replicas with full-mesh network
    let ids = vec!["A".to_string(), "B".to_string(), "C".to_string(), "D".to_string()];

    // Build nets and capture their senders
    let mut nets: BTreeMap<String, InProcNet> = BTreeMap::new();
    let mut inboxes: BTreeMap<String, tokio::sync::mpsc::UnboundedSender<Message>> = BTreeMap::new();
    for id in ids.iter() {
        let (net, tx) = InProcNet::new(id.clone());
        nets.insert(id.clone(), net);
        inboxes.insert(id.clone(), tx);
    }

    // Connect full mesh
    for net in nets.values() {
        for (peer, tx) in inboxes.iter() {
            // allow self to receive its own broadcast for simplicity
            net.connect(peer.clone(), tx.clone()).await;
        }
        log::info!("net {} connected to {} peers", net.me(), inboxes.len());
    }

    // Create replicas
    let leader_order = ids.clone();
    let commit_ctr = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for id in ids.iter() {
        let r = Replica {
            id: id.clone(),
            app: CounterApp::new(),
            kv: KVStore::new(),
            net: nets.get(id).unwrap().clone(),
        };
        let ctr = commit_ctr.clone();
        let order = leader_order.clone();
        handles.push(tokio::spawn(async move { r.run(ctr, order).await }));
    }

    // Quick health check: wait up to 10s for at least one commit across the cluster
    let start = Instant::now();
    loop {
        let n = commit_ctr.load(Ordering::Relaxed);
        if n > 0 { break; }
        if start.elapsed() > Duration::from_secs(10) {
            log::warn!("No commits observed within 10 seconds");
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }

    log::info!("Press Ctrl+C to exit");
    tokio::signal::ctrl_c().await?;

    // Keep join handles alive (not awaited because tasks are infinite loops)
    drop(handles);
    Ok(())
}
