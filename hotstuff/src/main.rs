mod app;
mod api;
mod kv;
mod net;

use anyhow::Result;
use app::OrderbookApp;
use kv::KVStore;
use net::{InProcNet, start_network_listener, connect_peer};

use base64::Engine;
use clap::Parser;
use hotstuff_rs::block_tree::accessors::public::BlockTreeCamera;
use hotstuff_rs::replica::{Configuration, ReplicaSpec};
use hotstuff_rs::types::data_types::{BufferSize, ChainID, EpochLength, Power};
use hotstuff_rs::types::update_sets::{AppStateUpdates, ValidatorSetUpdates};
use hotstuff_rs::types::validator_set::{SigningKey, ValidatorSet, ValidatorSetState, VerifyingKey};
use rand_core::OsRng;
use std::{fs, net::SocketAddr, path::PathBuf};
// no direct TCP reads/writes here; net.rs owns framing
use tokio::time::Duration;

#[derive(Parser, Debug, Clone)]
#[command(name = "hs-node", about = "HotStuff node with TCP P2P + REST API")] 
struct Args {
    #[arg(long)] key_file: Option<PathBuf>,
    #[arg(long, default_value = "127.0.0.1:9001")] p2p_listen: SocketAddr,
    #[arg(long)] peers: Vec<SocketAddr>,
    #[arg(long, default_value = "127.0.0.1:8081")] http_listen: SocketAddr,
    #[arg(long)] bootstrap_keys: Vec<PathBuf>,
    #[arg(long, default_value_t = 1)] bootstrap_power: u64,
    #[arg(long, default_value_t = false)] debug: bool,
    #[arg(long, default_value_t = false)] log_events: bool,
    #[arg(long, default_value = "warn")] hotstuff_log: String,
    #[arg(long, default_value_t = true)] forward_orders: bool,
    #[arg(long)] http_peers: Vec<String>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_logger(args.debug, &args.hotstuff_log);
    log::info!("Starting HotStuff node: p2p={}, http={}", args.p2p_listen, args.http_listen);

    // 1) Load or generate key
    let keypair: SigningKey = if let Some(path) = &args.key_file {
        if let Ok(b64) = fs::read_to_string(path) {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(b64.trim()) {
                let arr: [u8; 32] = bytes[..].try_into().unwrap_or([0u8; 32]);
                SigningKey::from_bytes(&arr)
            } else { let mut r = OsRng {}; SigningKey::generate(&mut r) }
        } else {
            let mut r = OsRng {};
            let k = SigningKey::generate(&mut r);
            let _ = fs::write(path, base64::engine::general_purpose::STANDARD_NO_PAD.encode(k.to_bytes()));
            k
        }
    } else { let mut r = OsRng {}; SigningKey::generate(&mut r) };
    let me_vk = keypair.verifying_key();

    // 2) Genesis validator set
    let init_as = AppStateUpdates::new();
    let mut all_vks: Vec<(VerifyingKey, Power)> = Vec::new();
    all_vks.push((me_vk, Power::new(args.bootstrap_power)));
    for path in &args.bootstrap_keys {
        if let Ok(b64) = fs::read_to_string(path) {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(b64.trim()) {
                let arr: [u8; 32] = bytes[..].try_into().unwrap_or([0u8; 32]);
                let sk = SigningKey::from_bytes(&arr);
                let vk = sk.verifying_key();
                if vk != me_vk { all_vks.push((vk, Power::new(args.bootstrap_power))); }
            }
        }
    }
    all_vks.sort_by(|a, b| a.0.to_bytes().cmp(&b.0.to_bytes()));
    let init_vs_updates = {
        let mut u = ValidatorSetUpdates::new();
        for (vk, p) in &all_vks { u.insert(*vk, *p); }
        u
    };
    let mut init_vs = ValidatorSet::new();
    init_vs.apply_updates(&init_vs_updates);
    let init_vs_state = ValidatorSetState::new(init_vs.clone(), init_vs, None, true);

    // 3) Networking: TCP listener + connect peers
    let (to_node_tx, to_node_rx) = tokio::sync::mpsc::unbounded_channel::<(VerifyingKey, hotstuff_rs::networking::messages::Message)>();
    start_network_listener(args.p2p_listen, to_node_tx.clone()).await?;
    let net = InProcNet::new(me_vk, to_node_rx, to_node_tx.clone());
    // Connect peers and add outbound channels; also discover VK via HTTP and map for unicast
    for peer in args.peers.clone().into_iter() {
        let net_clone = net.clone();
        let http_port = 8080 + (peer.port().saturating_sub(9000));
        let http_url = format!("http://{}:{}", peer.ip(), http_port);
        tokio::spawn(async move {
            loop {
                match connect_peer(peer).await {
                    Ok(tx) => {
                        net_clone.add_outbound(tx.clone()).await;
                        // Keep trying to discover vk until success, then map and exit
                        loop {
                            if let Some(vk) = discover_peer_vk(&http_url).await {
                                log::info!("discovered peer vk {} at {}", base64::engine::general_purpose::STANDARD_NO_PAD.encode(vk.to_bytes()), http_url);
                                net_clone.map_peer_vk(vk, tx.clone()).await;
                                break;
                            } else {
                                log::debug!("peer vk discovery pending at {}", http_url);
                            }
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                        break;
                    }
                    Err(_) => { tokio::time::sleep(Duration::from_secs(2)).await; }
                }
            }
        });
    }

    async fn discover_peer_vk(url: &str) -> Option<VerifyingKey> {
        // Use reqwest to avoid chunked parsing issues
        let full = format!("{}/me/vk", url.trim_end_matches('/'));
        match reqwest::get(&full).await {
            Ok(resp) => match resp.text().await {
                Ok(text) => {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(text.trim()) {
                        if bytes.len() == 32 {
                            if let Ok(vk) = VerifyingKey::from_bytes(&bytes[..].try_into().ok()?) { return Some(vk); }
                        }
                    }
                    None
                }
                Err(_) => None,
            },
            Err(_) => None,
        }
    }

    // 4) Build replica
    let kv = KVStore::new();
    hotstuff_rs::replica::Replica::initialize(kv.clone(), init_as.clone(), init_vs_state.clone());
    let camera0 = BlockTreeCamera::new(kv.clone());
    let vs_len = camera0.snapshot().committed_validator_set().expect("vs").len();
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

    let app = OrderbookApp::new();
    let app_http = app.clone();
    let replica = ReplicaSpec::builder()
        .app(app)
        .network(net)
        .kv_store(kv.clone())
        .configuration(cfg)
        .on_commit_block(|_| {})
        .build()
        .start();

    // 5) HTTP server
    let camera = replica.block_tree_camera().clone();
    let vk_b64 = base64::engine::general_purpose::STANDARD_NO_PAD.encode(me_vk.to_bytes());
    // Build HTTP peers list
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
    let kv_for_api = kv.clone();
    tokio::spawn(async move {
        api::serve_http(app_http, camera, kv_for_api, vk_b64, args.http_listen, args.forward_orders, http_peers).await;
    });

    // Background log latest committed payload for visibility
    {
        let camera = replica.block_tree_camera().clone();
        tokio::spawn(async move {
            let mut last: Option<Vec<u8>> = None;
            loop {
                let snap = camera.snapshot();
                if let Ok(Some(h)) = snap.highest_committed_block() {
                    if let Ok(Some(data)) = snap.block_data(&h) {
                        if let Some(d) = data.vec().get(0) {
                            let cur = d.bytes().to_vec();
                            if last.as_ref().map(|v| v != &cur).unwrap_or(true) {
                                last = Some(cur.clone());
                                log::info!("Committed payload: {}", base64::engine::general_purpose::STANDARD_NO_PAD.encode(&cur));
                            }
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });
    }

    tokio::signal::ctrl_c().await?;
    Ok(())
}

fn init_logger(debug: bool, hotstuff_level: &str) {
    let mut builder = env_logger::Builder::from_default_env();
    builder.format_timestamp_secs();
    let hs_lf = match hotstuff_level.to_lowercase().as_str() {
        "off" => log::LevelFilter::Off,
        "error" => log::LevelFilter::Error,
        "warn" => log::LevelFilter::Warn,
        "info" => log::LevelFilter::Info,
        "debug" => log::LevelFilter::Debug,
        "trace" => log::LevelFilter::Trace,
        _ => log::LevelFilter::Warn,
    };
    builder.filter_module("hotstuff_rs::logging", hs_lf);
    if debug { builder.filter_level(log::LevelFilter::Debug); } else { builder.filter_level(log::LevelFilter::Info); }
    builder.init();
}
