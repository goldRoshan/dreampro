use std::net::SocketAddr;

use axum::{extract::State, routing::{get, post}, Json, Router};
use base64::Engine;
use hotstuff_rs::block_tree::accessors::public::BlockTreeCamera;
use serde::Serialize;
use utoipa::ToSchema;

use crate::app::{Order, OrderInput, OrderbookApp};
use crate::kv::{KVStore, WriteBatch};
use hotstuff_rs::block_tree::pluggables::{KVStore as HsKVStore, WriteBatch as HsWriteBatch, KVGet};

#[derive(Clone)]
pub struct ApiState {
    app: OrderbookApp,
    camera: BlockTreeCamera<KVStore>,
    kv: KVStore,
    me_vk_b64: String,
    forward: bool,
    peers: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct StatusResponse {
    me: String,
    latest_block_b64: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct OrderbookLevel { pub price: f64, pub position: u64 }

#[derive(Serialize, ToSchema)]
pub struct OrderbookSymbol {
    pub symbol: String,
    pub buy: Vec<OrderbookLevel>,
    pub sell: Vec<OrderbookLevel>,
}

#[derive(Serialize, ToSchema)]
pub struct OrderbookResponse { pub symbols: Vec<OrderbookSymbol> }

/// GET /status
#[utoipa::path(get, path = "/status", responses((status = 200, body = StatusResponse)))]
async fn status(State(st): State<ApiState>) -> Json<StatusResponse> {
    let snap = st.camera.snapshot();
    let mut latest_block_b64 = None;
    if let Ok(Some(h)) = snap.highest_committed_block() {
        if let Ok(Some(data)) = snap.block_data(&h) {
            if let Some(datum) = data.vec().get(0) {
                latest_block_b64 = Some(base64::engine::general_purpose::STANDARD_NO_PAD.encode(datum.bytes()));
            }
        }
    }
    Json(StatusResponse { me: st.me_vk_b64.clone(), latest_block_b64 })
}

/// POST /order
#[utoipa::path(post, path = "/order", request_body = OrderInput, responses((status = 200)))]
async fn submit_order(State(st): State<ApiState>, Json(order): Json<OrderInput>) -> Json<&'static str> {
    let ord = Order::from_input(order.clone());
    st.app.submit_order(ord.clone());

    // Fast-path for single-node mode: apply directly to committed app state.
    if st
        .camera
        .snapshot()
        .committed_validator_set()
        .map(|vs| vs.len() == 1)
        .unwrap_or(false)
    {
        let mut wb = WriteBatch::new();
        // Update symbols list
        let mut symbols: Vec<String> = st
            .kv
            .snapshot()
            .get(b"book::symbols")
            .and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok())
            .unwrap_or_default();
        if !symbols.contains(&ord.symbol) { symbols.push(ord.symbol.clone()); symbols.sort(); }
        wb.set(b"book::symbols", &serde_json::to_vec(&symbols).unwrap());
        // Update side levels
        let key = if ord.side.eq_ignore_ascii_case("buy") { format!("book:{}:buy", ord.symbol) } else { format!("book:{}:sell", ord.symbol) };
        let mut levels: Vec<(i64, u64)> = st
            .kv
            .snapshot()
            .get(key.as_bytes())
            .and_then(|b| serde_json::from_slice::<Vec<(i64, u64)>>(&b).ok())
            .unwrap_or_default();
        let pk = (ord.price * 1_000_000.0).round() as i64;
        let mut found = false;
        for (p, pos) in levels.iter_mut() {
            if *p == pk { *pos += ord.position; found = true; break; }
        }
        if !found { levels.push((pk, ord.position)); }
        if ord.side.eq_ignore_ascii_case("buy") {
            levels.sort_by(|a, b| b.0.cmp(&a.0));
        } else {
            levels.sort_by(|a, b| a.0.cmp(&b.0));
        }
        wb.set(key.as_bytes(), &serde_json::to_vec(&levels).unwrap());
        let mut kv = st.kv.clone(); // KVStore write takes &mut self
        kv.write(wb);
    }

    // Forward the exact Order with its id to peers so they deduplicate and do not re-forward
    if st.forward && !st.peers.is_empty() {
        let peers = st.peers.clone();
        let ord_clone = ord.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            for p in peers {
                let _ = client
                    .post(format!("{}/order_forward", p))
                    .json(&ord_clone)
                    .send()
                    .await;
            }
        });
    }

    Json("accepted")
}

/// GET /orderbook
#[utoipa::path(get, path = "/orderbook", responses((status = 200, body = OrderbookResponse)))]
async fn committed_orderbook(State(st): State<ApiState>) -> Json<OrderbookResponse> {
    // Read committed app state from the block tree snapshot
    let snap = st.camera.snapshot();
    let mut out: Vec<OrderbookSymbol> = Vec::new();
    // Prefer committed app state; if absent (e.g., single-node fast-path), fall back to raw KV
    let mut symbols = snap
        .committed_app_state(b"book::symbols")
        .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
        .unwrap_or_default();
    if symbols.is_empty() {
        symbols = st
            .kv
            .snapshot()
            .get(b"book::symbols")
            .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
            .unwrap_or_default();
    }
    for sym in symbols.iter() {
        let buy_key = format!("book:{}:buy", sym);
        let sell_key = format!("book:{}:sell", sym);
        let mut buy: Vec<(i64, u64)> = snap
            .committed_app_state(buy_key.as_bytes())
            .and_then(|b| serde_json::from_slice::<Vec<(i64, u64)>>(&b).ok())
            .unwrap_or_default();
        if buy.is_empty() {
            buy = st
                .kv
                .snapshot()
                .get(buy_key.as_bytes())
                .and_then(|b| serde_json::from_slice::<Vec<(i64, u64)>>(&b).ok())
                .unwrap_or_default();
        }
        let buy = buy
            .into_iter()
            .map(|(pk, position)| OrderbookLevel { price: (pk as f64) / 1_000_000.0, position })
            .collect();
        let mut sell: Vec<(i64, u64)> = snap
            .committed_app_state(sell_key.as_bytes())
            .and_then(|b| serde_json::from_slice::<Vec<(i64, u64)>>(&b).ok())
            .unwrap_or_default();
        if sell.is_empty() {
            sell = st
                .kv
                .snapshot()
                .get(sell_key.as_bytes())
                .and_then(|b| serde_json::from_slice::<Vec<(i64, u64)>>(&b).ok())
                .unwrap_or_default();
        }
        let sell = sell
            .into_iter()
            .map(|(pk, position)| OrderbookLevel { price: (pk as f64) / 1_000_000.0, position })
            .collect();
        out.push(OrderbookSymbol { symbol: sym.clone(), buy, sell });
    }
    Json(OrderbookResponse { symbols: out })
}

/// Internal peer endpoint: accept forwarded Order with stable id. Not forwarded further.
#[utoipa::path(post, path = "/order_forward", request_body = Order, responses((status = 200)))]
async fn order_forward(State(st): State<ApiState>, Json(order): Json<Order>) -> Json<&'static str> {
    st.app.submit_order(order);
    Json("ok")
}

pub async fn serve_http(
    app: OrderbookApp,
    camera: BlockTreeCamera<KVStore>,
    kv: KVStore,
    me_vk_b64: String,
    listen: SocketAddr,
    forward: bool,
    peers: Vec<String>,
) {
    use utoipa::OpenApi;
    use utoipa_swagger_ui::SwaggerUi;

    #[derive(OpenApi)]
    #[openapi(
        paths(status, submit_order, committed_orderbook),
        components(schemas(StatusResponse, OrderInput, OrderbookResponse, OrderbookSymbol, OrderbookLevel))
    )]
    struct ApiDoc;

    let st = ApiState { app, camera, kv, me_vk_b64, forward, peers };

    let app = Router::new()
        .route("/status", get(status))
        .route("/order", post(submit_order))
        .route("/order_forward", post(order_forward))
        .route("/orderbook", get(committed_orderbook))
        .route("/me/vk", get(|State(st): State<ApiState>| async move { st.me_vk_b64 }))
        .route("/docs", get(|| async move { axum::response::Redirect::to("/swagger") }))
        .merge(SwaggerUi::new("/swagger").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(st);

    log::info!("HTTP listening on {}", listen);
    let listener = tokio::net::TcpListener::bind(listen).await.expect("bind");
    let _ = axum::serve(listener, app.into_make_service()).await;
}
