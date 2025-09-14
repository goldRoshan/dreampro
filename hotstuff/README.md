# HotStuff RS – In-Process 4-Replica Demo (WSL-ready)

This is a self-contained demo that runs four replicas in the same process using an in-proc "network" and an in-memory key-value store — fully wired to `hotstuff_rs = 0.4`. It logs commit events and includes a quick health check that ensures at least one commit within ~10 seconds.

## Prerequisites (WSL: Ubuntu 22.04+)

```bash
sudo apt update
sudo apt install -y build-essential git pkg-config clang libssl-dev
curl https://sh.rustup.rs -sSf | sh -s -- -y
source "$HOME/.cargo/env"
```

## Build & Run

```bash
cargo run
```

Set logging to see commit events:

```bash
RUST_LOG=info cargo run
```

You should see logs similar to:

```
INFO  hs-demo > Starting HotStuff demo (simulated) with 4 in-proc replicas
INFO  hs-demo > net A connected to 4 peers
...
INFO  hs-demo > replica=A committed view=2 payload=tick-5
```

Press `Ctrl+C` to exit.

## Project Layout

```
hs-demo/
  Cargo.toml
  README.md
  src/
    main.rs      # builds hotstuff_rs replicas and runs them
    app.rs       # CounterApp implementing hotstuff_rs::app::App
    kv.rs        # in-memory KV implementing hotstuff_rs::KVStore + WriteBatch
    net.rs       # in-proc Network implementing hotstuff_rs::Network
```

## Where to Plug Your Real App

- Replace the demo app with your logic in `src/app.rs` by implementing `hotstuff_rs::app::App` (deterministic, no wall-clock).
- Integrate a persistent KV in `src/kv.rs` by implementing `hotstuff_rs::block_tree::pluggables::KVStore` against e.g., RocksDB.
- Swap the in-proc network with a real transport in `src/net.rs` by implementing `hotstuff_rs::networking::network::Network`.

The binary in `main.rs` shows how to:
- Build the initial `ValidatorSet`,
- Initialize the block tree via `Replica::initialize`,
- Build a `Configuration` (chain, timeouts, epoch), and
- Start replicas with `ReplicaSpec::builder().start()`.
