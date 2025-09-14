mod app;
mod kv;
mod net;

use anyhow::Result;
use app::CounterApp;
use kv::KVStore;
use log;
use net::InProcNet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration, Instant};

use base64::Engine; // for .encode on base64 engines
use hotstuff_rs::replica::{Configuration, Replica, ReplicaSpec};
use hotstuff_rs::types::data_types::{BufferSize, ChainID, EpochLength, Power};
use hotstuff_rs::types::update_sets::{AppStateUpdates, ValidatorSetUpdates};
use hotstuff_rs::types::validator_set::{SigningKey, ValidatorSet, ValidatorSetState};
use rand_core::OsRng;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("Starting HotStuff-rs 4-replica in-proc demo");

    // 1) Generate 4 keypairs and build a full-mesh in-proc network
    let mut csprg = OsRng {};
    let keypairs: Vec<SigningKey> = (0..4).map(|_| SigningKey::generate(&mut csprg)).collect();
    let verifying_keys: Vec<_> = keypairs.iter().map(|k| k.verifying_key()).collect();
    let nets = InProcNet::full_mesh(verifying_keys.iter().cloned());
    for n in &nets { log::info!("net {} connected to {} peers", base64::engine::general_purpose::STANDARD_NO_PAD.encode(n.me().to_bytes()), verifying_keys.len()); }

    // 2) Initial states
    let init_as = AppStateUpdates::new();
    let init_vs_updates = {
        let mut u = ValidatorSetUpdates::new();
        for vk in &verifying_keys { u.insert(*vk, Power::new(1)); }
        u
    };
    let mut init_vs = ValidatorSet::new();
    init_vs.apply_updates(&init_vs_updates);
    let init_vs_state = ValidatorSetState::new(init_vs.clone(), init_vs, None, true);

    // 3) Build configs, initialize storage, start replicas
    let commit_ctr = Arc::new(AtomicUsize::new(0));
    let mut replicas: Vec<Replica<KVStore>> = Vec::new();
    for ((kp, net), idx) in keypairs.into_iter().zip(nets.into_iter()).zip(0..) {
        // Per-replica KV and initialization
        let mut kv = KVStore::new();
        Replica::initialize(kv.clone(), init_as.clone(), init_vs_state.clone());

        let cfg = Configuration::builder()
            .me(kp)
            .chain_id(ChainID::new(0))
            .block_sync_request_limit(8)
            .block_sync_server_advertise_time(Duration::from_secs(10))
            .block_sync_response_timeout(Duration::from_secs(3))
            .block_sync_blacklist_expiry_time(Duration::from_secs(10))
            .block_sync_trigger_min_view_difference(2)
            .block_sync_trigger_timeout(Duration::from_secs(60))
            .progress_msg_buffer_capacity(BufferSize::new(1024))
            .epoch_length(EpochLength::new(50))
            .max_view_time(Duration::from_millis(1500))
            .log_events(true)
            .build();

        let ctr = commit_ctr.clone();
        let replica = ReplicaSpec::builder()
            .app(CounterApp::new())
            .network(net)
            .kv_store(kv)
            .configuration(cfg)
            .on_commit_block(move |ev| {
                log::info!("Committed block: {}", base64::engine::general_purpose::STANDARD_NO_PAD.encode(ev.block.bytes()));
                ctr.fetch_add(1, Ordering::Relaxed);
            })
            .build()
            .start();

        log::info!("Replica {} started", idx);
        replicas.push(replica);
    }

    // 4) Quick health check
    let start = Instant::now();
    while commit_ctr.load(Ordering::Relaxed) == 0 && start.elapsed() < Duration::from_secs(10) {
        sleep(Duration::from_millis(100)).await;
    }
    if commit_ctr.load(Ordering::Relaxed) == 0 {
        log::warn!("No commits observed within 10 seconds");
    } else {
        log::info!("At least one block committed");
    }

    log::info!("Press Ctrl+C to exit");
    tokio::signal::ctrl_c().await?;
    drop(replicas);
    Ok(())
}
