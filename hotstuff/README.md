# HotStuff RS – In-Process 4-Replica Demo (WSL-ready)

<<<<<<< HEAD
This is a self-contained demo that runs four replicas in the same process using an in-proc "network" and an in-memory key-value store — fully wired to `hotstuff_rs = 0.4`. It logs commit events and includes a quick health check that ensures at least one commit within ~10 seconds.
=======
This is a self-contained demo that runs four replicas in the same process using an in-proc "network" and an in-memory key-value store. It logs commit events and includes a quick health check that ensures at least one commit within ~10 seconds.

Note: The code includes `hotstuff_rs = "0.4"` as a dependency per requirements. The current demo path simulates consensus in-process to keep the example runnable everywhere without external networking. You can later replace the demo app and network with real implementations against the HotStuff crate.
>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)

## Prerequisites (WSL: Ubuntu 22.04+)

```bash
sudo apt update
sudo apt install -y build-essential git pkg-config clang libssl-dev
curl https://sh.rustup.rs -sSf | sh -s -- -y
source "$HOME/.cargo/env"
```

<<<<<<< HEAD
## Build & Run (single process demo)
=======
## Build & Run
>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)

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

<<<<<<< HEAD
## Multi-Process Nodes (TCP + HTTP)

This binary can run several separate processes (one per node) that communicate over TCP and expose an HTTP API.

### CLI flags

- `--key-file <path>`: Base64-encoded 32-byte secret. If the file doesn’t exist, a new key is generated and written.
- `--p2p-listen <ip:port>`: TCP address for HotStuff messages (borsh-serialized).
- `--peers <ip:port>`: Repeatable. Peer addresses to connect to (auto-reconnects every few seconds).
- `--http-listen <ip:port>`: HTTP server with Swagger UI.
- `--bootstrap-keys <path>`: Repeatable. Additional key files to include in the genesis validator set.
- `--bootstrap-power <u64>`: Voting power for bootstrap keys (default 1).

### Start a 4-node cluster

1) Prepare keys (or let the node create them on first run):

```bash
echo "$(openssl rand -base64 32 | tr -d '\n' | sed 's/=//g')" > node1.key
echo "$(openssl rand -base64 32 | tr -d '\n' | sed 's/=//g')" > node2.key
echo "$(openssl rand -base64 32 | tr -d '\n' | sed 's/=//g')" > node3.key
echo "$(openssl rand -base64 32 | tr -d '\n' | sed 's/=//g')" > node4.key
```

2) Start node 1 (leader) and bootstrap all validators at genesis:

```bash
RUST_LOG=info cargo run -p hs-demo -- \
  --key-file node1.key \
  --bootstrap-keys node2.key --bootstrap-keys node3.key --bootstrap-keys node4.key \
  --bootstrap-power 1 \
  --p2p-listen 127.0.0.1:9001 \
  --http-listen 127.0.0.1:8081 \
  --peers 127.0.0.1:9002 --peers 127.0.0.1:9003 --peers 127.0.0.1:9004
```

3) Start nodes 2–4 with full peering:

```bash
./script/start.sh --node 2
./script/start.sh --node 3
./script/start.sh --node 4
```

The node auto-reconnects to peers if a connection drops. Pacemaker `max_view_time` is set to 200 ms; commits typically appear every ~600 ms in the happy path.

### HTTP API (Swagger)

- Swagger UI: `http://<http_listen>/docs`
- Endpoints:
  - `POST /submit-order` → { symbol, price, side, position }
  - `GET /committed-order-book/{symbol}` → consensus order book from last committed block
  - `GET /order-book/{symbol}` → pending (local) orders (for debugging)
  - `POST /add-validator` → { vk_b64, power } queue a validator insert for the next block
  - `GET /me/vk` → this node’s verifying key (base64)

Tip: Submit orders to the leader’s HTTP. All nodes will show the same committed book via `GET /committed-order-book/{symbol}`.

### Helper script

Use the helper script to simplify node startup:

```bash
# Create keys for four nodes and start node 1
bash script/start.sh --node 1 --create_new_key

# Start other nodes
bash script/start.sh --node 2
bash script/start.sh --node 3
bash script/start.sh --node 4
```

The script computes sensible defaults for ports (P2P: 900<id>, HTTP: 808<id>), ensures keys exist (or creates when `--create_new_key` is passed), bootstraps validators on node 1, and connects to other peers.

### Failover

If the leader goes down, the pacemaker rotates to the next leader after a timeout. Keep a robust validator set (e.g., 4 validators → f=1) and ensure each node peers to the others to maintain liveness.

=======
>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)
## Project Layout

```
hs-demo/
  Cargo.toml
  README.md
  src/
<<<<<<< HEAD
    main.rs      # single node binary (key-file, p2p/http listen, peers, bootstrap-keys)
    app.rs       # CounterApp; block payload encodes orders + validator set updates
    kv.rs        # in-memory KV implementing hotstuff_rs::KVStore + WriteBatch
    net.rs       # TCP Network implementing hotstuff_rs::Network (borsh, auto-reconnect)
    http.rs      # Axum HTTP server + Swagger
=======
    main.rs      # wires up 4 replicas and runs them
    app.rs       # CounterApp (deterministic app logic)
    kv.rs        # in-memory KV with atomic batches & snapshots
    net.rs       # in-proc network using unbounded mpsc channels
>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)
```

## Where to Plug Your Real App

<<<<<<< HEAD
- Replace the demo app with your logic in `src/app.rs` by implementing `hotstuff_rs::app::App` (deterministic, no wall-clock).
- Integrate a persistent KV in `src/kv.rs` by implementing `hotstuff_rs::block_tree::pluggables::KVStore` against e.g., RocksDB.
- Swap the in-proc network with a real transport in `src/net.rs` by implementing `hotstuff_rs::networking::network::Network`.

The binary in `main.rs` shows how to:
- Build the initial `ValidatorSet`,
- Initialize the block tree via `Replica::initialize`,
- Build a `Configuration` (chain, timeouts, epoch), and
- Start replicas with `ReplicaSpec::builder().start()`.
=======
- Replace the demo app with your logic in `src/app.rs`. Keep methods deterministic and fast.
- Integrate your persistent KV in `src/kv.rs` (e.g., RocksDB). This demo uses an in-memory BTreeMap.
- Swap the in-proc network with a real transport in `src/net.rs` (TCP/UDP/quic/grpc). The key requirement is a non-blocking `recv()` that returns `Option`.

## Notes on HotStuff Integration

- The crate `hotstuff_rs = "0.4"` is included now. To integrate it fully:
  - Implement the crate's `App`, `KVStore`, and `Network` traits in `src/app.rs`, `src/kv.rs`, and `src/net.rs` respectively.
  - In `src/main.rs`, build the validator set, configuration, pacemaker, and replicas using the crate's builder/API.
  - Wire the commit callback to log commit events (see the `on_commit` closure in the goal description).

This repository provides a clean, modular starting point so you can swap the simulated consensus with the real `hotstuff_rs` wiring with minimal code movement.

>>>>>>> 5f9cdcd (Move demo into hotstuff/ subfolder and add root workspace manifest)
