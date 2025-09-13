# HotStuff RS – In-Process 4-Replica Demo (WSL-ready)

This is a self-contained demo that runs four replicas in the same process using an in-proc "network" and an in-memory key-value store. It logs commit events and includes a quick health check that ensures at least one commit within ~10 seconds.

Note: The code includes `hotstuff_rs = "0.4"` as a dependency per requirements. The current demo path simulates consensus in-process to keep the example runnable everywhere without external networking. You can later replace the demo app and network with real implementations against the HotStuff crate.

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
    main.rs      # wires up 4 replicas and runs them
    app.rs       # CounterApp (deterministic app logic)
    kv.rs        # in-memory KV with atomic batches & snapshots
    net.rs       # in-proc network using unbounded mpsc channels
```

## Where to Plug Your Real App

- Replace the demo app with your logic in `src/app.rs`. Keep methods deterministic and fast.
- Integrate your persistent KV in `src/kv.rs` (e.g., RocksDB). This demo uses an in-memory BTreeMap.
- Swap the in-proc network with a real transport in `src/net.rs` (TCP/UDP/quic/grpc). The key requirement is a non-blocking `recv()` that returns `Option`.

## Notes on HotStuff Integration

- The crate `hotstuff_rs = "0.4"` is included now. To integrate it fully:
  - Implement the crate's `App`, `KVStore`, and `Network` traits in `src/app.rs`, `src/kv.rs`, and `src/net.rs` respectively.
  - In `src/main.rs`, build the validator set, configuration, pacemaker, and replicas using the crate's builder/API.
  - Wire the commit callback to log commit events (see the `on_commit` closure in the goal description).

This repository provides a clean, modular starting point so you can swap the simulated consensus with the real `hotstuff_rs` wiring with minimal code movement.

