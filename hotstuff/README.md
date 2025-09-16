# HotStuff-RS – 4-Node Consensus with REST + Swagger

This project runs a 4-node HotStuff BFT cluster (single process, in-proc network) using `hotstuff_rs = 0.4`. Each node exposes a REST API with Swagger to submit orders and retrieve the latest committed aggregated orderbook.

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
INFO  hs-demo > Starting HotStuff-RS 4-node cluster with REST + Swagger
...
INFO  hs-demo > Cluster running. Swagger at http://127.0.0.1:8081/swagger (and :8082-:8084)
```

Press `Ctrl+C` to exit.

## REST API (Swagger)

- Swagger UI: `http://127.0.0.1:8081/swagger` (also 8082, 8083, 8084)
- Endpoints:
  - `GET /status` → node info (leader hint, latest committed block)
  - `POST /order` → submit `{symbol, side, position, price}` to any node
  - `GET /orderbook` → latest committed aggregated orderbook across all symbols

Orders submitted to any node are forwarded to all peers (best-effort) so the leader can include them in the next block. The committed orderbook is kept in HotStuff app state and served from the latest committed snapshot.
## Project Layout

```
hs-demo/
  Cargo.toml
  README.md
  src/
    main.rs      # boots 4 replicas, HTTP per node
    app.rs       # OrderbookApp (deterministic app logic; aggregates by price/side)
    api.rs       # Axum HTTP + Swagger
    kv.rs        # in-memory KV implementing hotstuff_rs::KVStore
    net.rs       # in-proc network implementing hotstuff_rs::Network
```

## Where to Plug Your Real App

## Extending

- Swap `KVStore` with a persistent backend (e.g., RocksDB) by implementing `hotstuff_rs::block_tree::pluggables::KVStore`.
- Replace the in-proc network in `net.rs` with a real transport by implementing `hotstuff_rs::networking::network::Network`.
- Customize `OrderbookApp` to fit your business logic by implementing `hotstuff_rs::app::App`.
