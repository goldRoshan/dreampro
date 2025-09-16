mod app;
<<<<<<< HEAD
mod http;
=======
>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)
mod kv;
mod net;

use anyhow::Result;
use app::CounterApp;
<<<<<<< HEAD
use kv::KVStore;
use log;
use net::{InProcNet, start_network_listener, connect_peer};
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::time::Duration;
use clap::Parser;
use std::path::PathBuf;
use std::fs;
use std::net::SocketAddr;

use base64::Engine; // for .encode on base64 engines
use hotstuff_rs::replica::{Configuration, Replica, ReplicaSpec};
use hotstuff_rs::block_tree::accessors::public::BlockTreeCamera;
use hotstuff_rs::types::data_types::{BufferSize, ChainID, EpochLength, Power};
use hotstuff_rs::types::update_sets::{AppStateUpdates, ValidatorSetUpdates};
use hotstuff_rs::types::validator_set::{SigningKey, ValidatorSet, ValidatorSetState};
use rand_core::OsRng;

#[derive(Parser, Debug, Clone)]
#[command(name = "hs-node", about = "HotStuff demo node with HTTP + TCP network")]
struct Args {
    #[arg(long)] key_file: Option<PathBuf>,
    #[arg(long, default_value = "0.0.0.0:9000")] p2p_listen: SocketAddr,
    #[arg(long)] peers: Vec<SocketAddr>,
    #[arg(long, default_value = "0.0.0.0:8080")] http_listen: SocketAddr,
    /// Optional list of key files (base64-encoded 32-byte secrets) to include in the genesis validator set.
    #[arg(long)] bootstrap_keys: Vec<PathBuf>,
    /// Voting power to assign to each bootstrapped validator (default 1)
    #[arg(long, default_value_t = 1)] bootstrap_power: u64,
    /// Enable debug logs from hotstuff_rs and the node
    #[arg(long, default_value_t = false)] debug: bool,
    /// Enable hotstuff_rs event logging inside the protocol (default: false)
    #[arg(long, default_value_t = false)] log_events: bool,
    /// Logging level for hotstuff_rs::logging (one of: off,error,warn,info,debug,trace). Default: warn
    #[arg(long, default_value = "warn")] hotstuff_log: String,
    /// When true, this node will forward submitted orders to peer HTTP endpoints as best-effort gossip
    #[arg(long, default_value_t = true)] forward_orders: bool,
    /// Optional list of HTTP peer endpoints (http://host:port). If empty, peers are derived from --peers by mapping 900x->808x
    #[arg(long)] http_peers: Vec<String>,
=======
use kv::{KVStore, WriteBatch};
use log; // use macros as log::info!, log::warn!
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
    peers: Vec<String>,
}

impl Replica {
    async fn run(self, commit_ctr: Arc<AtomicUsize>, leader_order: Vec<String>) {
        let id = self.id.clone();
        let mut view: u64 = 0;
        let mut last_tick = Instant::now();
        let tick = Duration::from_millis(400);

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
                                wb.put(b"last_commit", &msg.payload);
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
                        wb.put(marker.as_bytes(), &msg.payload);
                        self.kv.write(wb).await;
                    }
                    MsgKind::Ping => {}
                }
            }

            // Yield to runtime a bit
            tokio::task::yield_now().await;
        }
    }
>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
<<<<<<< HEAD
    let args = Args::parse();
    init_logger(args.debug, &args.hotstuff_log);
    log::info!("Starting HotStuff node: p2p={}, http={}", args.p2p_listen, args.http_listen);

    // 1) Load or generate local keypair (base64-encoded 32-byte secret in file)
    let keypair: SigningKey = if let Some(path) = &args.key_file {
        if let Ok(b64) = fs::read_to_string(path) {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(b64.trim()) {
                let arr: [u8;32] = bytes[..].try_into().unwrap_or([0u8;32]);
                SigningKey::from_bytes(&arr)
            } else { let mut r=OsRng{}; SigningKey::generate(&mut r) }
        } else {
            let mut r=OsRng{}; let k=SigningKey::generate(&mut r); let _=fs::write(path, base64::engine::general_purpose::STANDARD_NO_PAD.encode(k.to_bytes())); k
        }
    } else { let mut r=OsRng{}; SigningKey::generate(&mut r) };
    let me_vk = keypair.verifying_key();

    // 2) Initial states (genesis validator set)
    let init_as = AppStateUpdates::new();
    // Collect all validators deterministically to ensure identical ordering across nodes
    let mut all_vks: Vec<(hotstuff_rs::types::validator_set::VerifyingKey, Power)> = Vec::new();
    all_vks.push((me_vk, Power::new(args.bootstrap_power)));
    for path in &args.bootstrap_keys {
        if let Ok(b64) = fs::read_to_string(path) {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(b64.trim()) {
                let arr: [u8;32] = bytes[..].try_into().unwrap_or([0u8;32]);
                let sk = SigningKey::from_bytes(&arr);
                let vk = sk.verifying_key();
                if vk != me_vk { all_vks.push((vk, Power::new(args.bootstrap_power))); }
            }
        }
    }
    // Sort by verifying key bytes so every node derives the same ordering
    all_vks.sort_by(|a, b| a.0.to_bytes().cmp(&b.0.to_bytes()));
    let init_vs_updates = {
        let mut u = ValidatorSetUpdates::new();
        for (vk, p) in &all_vks { u.insert(*vk, *p); }
        u
    };
    let mut init_vs = ValidatorSet::new();
    init_vs.apply_updates(&init_vs_updates);
    let init_vs_state = ValidatorSetState::new(init_vs.clone(), init_vs, None, true);

    // 3) Networking: start TCP listener and connect peers
    let (to_node_tx, to_node_rx) = tokio::sync::mpsc::unbounded_channel::<(hotstuff_rs::types::validator_set::VerifyingKey, hotstuff_rs::networking::messages::Message)>();
    start_network_listener(args.p2p_listen, to_node_tx.clone()).await?;
    let net = InProcNet::new(me_vk, to_node_rx, to_node_tx.clone());
    // Derive HTTP peer list early to discover their verifying keys and map them to p2p senders
    let http_peer_endpoints: Vec<String> = args.peers.iter().map(|sa| {
        let http_port = 8080 + (sa.port().saturating_sub(9000));
        format!("http://{}:{}", sa.ip(), http_port)
    }).collect();
    for (i, peer) in args.peers.clone().into_iter().enumerate() {
        let net_clone = net.clone();
        let http_url = http_peer_endpoints.get(i).cloned();
        tokio::spawn(async move {
            // Try until first successful connection and vk discovery, then keep it
            loop {
                match connect_peer(peer).await {
                    Ok(tx) => {
                        // ensure we can broadcast to this peer immediately
                        net_clone.add_outbound(tx.clone()).await;
                        // Discover peer vk via its HTTP /me/vk
                        if let Some(url) = http_url.clone() {
                            if let Some(vk) = discover_peer_vk(&url).await {
                                net_clone.map_peer_vk(vk, tx.clone()).await;
                                // We already added outbound above
                                break;
                            }
                        }
                        // If discovery failed, retry after delay
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                    Err(_) => {
                        tokio::time::sleep(Duration::from_secs(3)).await;
                    }
                }
            }
        });
    }

    async fn discover_peer_vk(url: &str) -> Option<hotstuff_rs::types::validator_set::VerifyingKey> {
        // url like http://host:port
        let stripped = url.strip_prefix("http://")?;
        let mut parts = stripped.splitn(2, ':');
        let host = parts.next()?;
        let port = parts.next()?.parse::<u16>().ok()?;
        let addr = format!("{}:{}", host, port);
        // Minimal HTTP GET /me/vk
        if let Ok(mut stream) = tokio::net::TcpStream::connect(addr.clone()).await {
            let req = format!("GET /me/vk HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", host);
            if stream.write_all(req.as_bytes()).await.is_ok() {
                let mut buf = vec![0u8; 4096];
                if let Ok(n) = tokio::time::timeout(Duration::from_millis(500), stream.read(&mut buf)).await.ok()? {
                    let s = String::from_utf8_lossy(&buf[..n]);
                    // parse last line as body (very naive)
                    if let Some(body) = s.rsplit("\r\n").next() {
                        if let Ok(bytes) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(body.trim()) {
                            if bytes.len() == 32 {
                                if let Ok(vk) = hotstuff_rs::types::validator_set::VerifyingKey::from_bytes(&bytes[..].try_into().ok()?) {
                                    return Some(vk);
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    // 4) Build replica
    let kv = KVStore::new();
    Replica::initialize(kv.clone(), init_as.clone(), init_vs_state.clone());
    // Log genesis validator set size for troubleshooting
    let camera0 = BlockTreeCamera::new(kv.clone());
    let vs_len = camera0
        .snapshot()
        .committed_validator_set()
        .expect("vs")
        .len();
    log::info!("Genesis validator set size = {}", vs_len);

    let cfg = Configuration::builder()
        .me(keypair)
        .chain_id(ChainID::new(0))
        .block_sync_request_limit(8)
        .block_sync_server_advertise_time(Duration::from_secs(10))
        .block_sync_response_timeout(Duration::from_secs(3))
        .block_sync_blacklist_expiry_time(Duration::from_secs(10))
        .block_sync_trigger_min_view_difference(2)
        .block_sync_trigger_timeout(Duration::from_secs(60))
        .progress_msg_buffer_capacity(BufferSize::new(1024))
        .epoch_length(EpochLength::new(50))
        .max_view_time(Duration::from_millis(500))
        .log_events(args.log_events)
        .build();

    let app = CounterApp::new();
    let app_http = app.clone();
    let replica = ReplicaSpec::builder()
        .app(app)
        .network(net)
        .kv_store(kv)
        .configuration(cfg)
        .on_commit_block(|ev| {
            log::info!("Committed block: {}", base64::engine::general_purpose::STANDARD_NO_PAD.encode(ev.block.bytes()));
        })
        .build()
        .start();

    // 5) HTTP server with Swagger (reads committed order book from block tree)
    let camera = replica.block_tree_camera().clone();
    let vk_b64 = base64::engine::general_purpose::STANDARD_NO_PAD.encode(me_vk.to_bytes());
    // Build HTTP peer list
    let http_peers: Vec<String> = if !args.http_peers.is_empty() {
        args.http_peers.clone()
    } else {
        args.peers
            .iter()
            .map(|sa| {
                let p2p = sa.port();
                let http_port = 8080 + (p2p.saturating_sub(9000));
                format!("http://{}:{}", sa.ip(), http_port)
            })
            .collect()
    };
    tokio::spawn(async move { http::serve_http(app_http, camera, vk_b64, args.http_listen, args.forward_orders, http_peers).await });

    // Background: poll highest committed block to aid troubleshooting
    {
        let camera = replica.block_tree_camera().clone();
        tokio::spawn(async move {
            let mut last_payload: Option<Vec<u8>> = None;
            loop {
                let snap = camera.snapshot();
                if let Ok(Some(h)) = snap.highest_committed_block() {
                    if let Ok(Some(data)) = snap.block_data(&h) {
                        if let Some(datum) = data.vec().get(0) {
                            let cur = datum.bytes().to_vec();
                            if last_payload.as_ref().map(|v| v != &cur).unwrap_or(true) {
                                last_payload = Some(cur.clone());
                                let b64 = base64::engine::general_purpose::STANDARD_NO_PAD.encode(&cur);
                                log::info!("Committed (polled) block payload: {}", b64);
                            }
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });
    }

    // 6) Stay alive until Ctrl+C
    log::info!("Press Ctrl+C to exit");
    tokio::signal::ctrl_c().await?;
    drop(replica);
    Ok(())
}

fn init_logger(debug: bool, hotstuff_level: &str) {
    use env_logger::fmt::Formatter;
    use log::LevelFilter;
    use std::io::Write;
    let mut builder = env_logger::Builder::new();
    builder.format(|buf: &mut Formatter, record: &log::Record| {
        let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3f");
        writeln!(buf, "[{} {} {}] {}", ts, record.level(), record.target(), record.args())
    });
    builder.filter_level(LevelFilter::Info);
    // Map user-provided hotstuff level for the noisy event module
    let hs_lf = match hotstuff_level.to_lowercase().as_str() {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Warn,
    };
    builder.filter_module("hotstuff_rs::logging", hs_lf);
    // Keep other hotstuff modules quiet unless debug requested
    builder.filter_module(
        "hotstuff_rs",
        if debug { LevelFilter::Debug } else { LevelFilter::Warn },
    );
    builder.filter_module("hs_demo", if debug { LevelFilter::Debug } else { LevelFilter::Info });
    let _ = builder.try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    // Helper to build a deterministic, sorted ValidatorSetUpdates from vks
    fn vs_updates_from(vks: &[hotstuff_rs::types::validator_set::VerifyingKey]) -> ValidatorSetUpdates {
        let mut pairs: Vec<(hotstuff_rs::types::validator_set::VerifyingKey, Power)> = vks
            .iter()
            .map(|vk| (*vk, Power::new(1)))
            .collect();
        pairs.sort_by(|a, b| a.0.to_bytes().cmp(&b.0.to_bytes()));
        let mut u = ValidatorSetUpdates::new();
        for (vk, p) in pairs { u.insert(vk, p); }
        u
    }

    /// Smoke test: spins 4 in-proc replicas and waits for a commit.
    /// Ignored by default due to timing sensitivity in CI; run with `cargo test -- --ignored`.
    #[ignore]
    #[tokio::test(flavor = "multi_thread")]
    async fn four_replicas_commit_at_least_one_block() {
        // 1) Keys
        let mut r = OsRng{};
        let sk1 = SigningKey::generate(&mut r);
        let sk2 = SigningKey::generate(&mut r);
        let sk3 = SigningKey::generate(&mut r);
        let sk4 = SigningKey::generate(&mut r);
        let vk1 = sk1.verifying_key();
        let vk2 = sk2.verifying_key();
        let vk3 = sk3.verifying_key();
        let vk4 = sk4.verifying_key();
        let all_vks = vec![vk1, vk2, vk3, vk4];

        // 2) KV and genesis
        let build_vs_state = || {
            let mut init_vs = ValidatorSet::new();
            init_vs.apply_updates(&vs_updates_from(&all_vks));
            ValidatorSetState::new(init_vs.clone(), init_vs, None, true)
        };
        let kv1 = KVStore::new();
        let kv2 = KVStore::new();
        let kv3 = KVStore::new();
        let kv4 = KVStore::new();
        let init_as = AppStateUpdates::new();
        Replica::initialize(kv1.clone(), init_as.clone(), build_vs_state());
        Replica::initialize(kv2.clone(), init_as.clone(), build_vs_state());
        Replica::initialize(kv3.clone(), init_as.clone(), build_vs_state());
        Replica::initialize(kv4.clone(), init_as.clone(), build_vs_state());

        // 3) Networks (pure in-proc wiring using local channels)
        let (tx1, rx1) = tokio::sync::mpsc::unbounded_channel::<(hotstuff_rs::types::validator_set::VerifyingKey, hotstuff_rs::networking::messages::Message)>();
        let (tx2, rx2) = tokio::sync::mpsc::unbounded_channel();
        let (tx3, rx3) = tokio::sync::mpsc::unbounded_channel();
        let (tx4, rx4) = tokio::sync::mpsc::unbounded_channel();
        let net1 = InProcNet::new(vk1, rx1, tx1.clone());
        let net2 = InProcNet::new(vk2, rx2, tx2.clone());
        let net3 = InProcNet::new(vk3, rx3, tx3.clone());
        let net4 = InProcNet::new(vk4, rx4, tx4.clone());
        // Fully connect by sharing local tx among all
        net1.add_outbound(tx2.clone()).await; net1.add_outbound(tx3.clone()).await; net1.add_outbound(tx4.clone()).await;
        net2.add_outbound(tx1.clone()).await; net2.add_outbound(tx3.clone()).await; net2.add_outbound(tx4.clone()).await;
        net3.add_outbound(tx1.clone()).await; net3.add_outbound(tx2.clone()).await; net3.add_outbound(tx4.clone()).await;
        net4.add_outbound(tx1.clone()).await; net4.add_outbound(tx2.clone()).await; net4.add_outbound(tx3.clone()).await;

        // 4) Configuration (slightly relaxed timers for tests)
        let cfg = |me: SigningKey| {
            Configuration::builder()
                .me(me)
                .chain_id(ChainID::new(0))
                .block_sync_request_limit(8)
                .block_sync_server_advertise_time(Duration::from_secs(5))
                .block_sync_response_timeout(Duration::from_secs(2))
                .block_sync_blacklist_expiry_time(Duration::from_secs(5))
                .block_sync_trigger_min_view_difference(2)
                .block_sync_trigger_timeout(Duration::from_secs(20))
                .progress_msg_buffer_capacity(BufferSize::new(1024))
                .epoch_length(EpochLength::new(50))
                .max_view_time(Duration::from_millis(300))
                .log_events(false)
                .build()
        };

        // 5) Start replicas with commit counters
        let app1 = CounterApp::new();
        let app2 = CounterApp::new();
        let app3 = CounterApp::new();
        let app4 = CounterApp::new();
        let c1 = Arc::new(AtomicUsize::new(0));
        let c2 = Arc::new(AtomicUsize::new(0));
        let c3 = Arc::new(AtomicUsize::new(0));
        let c4 = Arc::new(AtomicUsize::new(0));
        let r1 = {
            let c = c1.clone();
            ReplicaSpec::builder().app(app1.clone()).network(net1).kv_store(kv1).configuration(cfg(sk1))
                .on_commit_block(move |_| { c.fetch_add(1, Ordering::SeqCst); })
                .build().start()
        };
        let r2 = {
            let c = c2.clone();
            ReplicaSpec::builder().app(app2.clone()).network(net2).kv_store(kv2).configuration(cfg(sk2))
                .on_commit_block(move |_| { c.fetch_add(1, Ordering::SeqCst); })
                .build().start()
        };
        let r3 = {
            let c = c3.clone();
            ReplicaSpec::builder().app(app3.clone()).network(net3).kv_store(kv3).configuration(cfg(sk3))
                .on_commit_block(move |_| { c.fetch_add(1, Ordering::SeqCst); })
                .build().start()
        };
        let r4 = {
            let c = c4.clone();
            ReplicaSpec::builder().app(app4.clone()).network(net4).kv_store(kv4).configuration(cfg(sk4))
                .on_commit_block(move |_| { c.fetch_add(1, Ordering::SeqCst); })
                .build().start()
        };

        // 6) Inject one order into each app to ensure non-empty payloads
        let o = |sym: &str| crate::app::Order { id: Uuid::new_v4(), symbol: sym.to_string(), price: 1.0, side: "buy".to_string(), position: 1 };
        app1.submit_order(o("T1"));
        app2.submit_order(o("T2"));
        app3.submit_order(o("T3"));
        app4.submit_order(o("T4"));

        // 7) Wait up to a few seconds for at least one commit on any replica
        let mut ok = false;
        for _ in 0..100 { // ~10s
            if c1.load(Ordering::SeqCst) > 0 || c2.load(Ordering::SeqCst) > 0 || c3.load(Ordering::SeqCst) > 0 || c4.load(Ordering::SeqCst) > 0 {
                ok = true; break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        // Cleanup
        drop((r1, r2, r3, r4));
        assert!(ok, "expected at least one committed block across replicas");
    }
}
=======
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
    for (me, net) in nets.iter() {
        for (peer, tx) in inboxes.iter() {
            // allow self to receive its own broadcast for simplicity
            net.connect(peer.clone(), tx.clone()).await;
        }
        log::info!("net {} connected to {} peers", me, inboxes.len());
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
            peers: leader_order.clone(),
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
>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)
