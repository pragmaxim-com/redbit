use demo::storage::empty_temp_storage;
use demo::*;
use redbit::*;
use std::net::SocketAddr;
use std::sync::Arc;
use syncer::combine;
use tokio::sync::watch;
use tower_http::cors;

#[tokio::main]
async fn main() {
    let storage = empty_temp_storage("redbit", 1);
    let cors = cors::CorsLayer::new()
        .allow_origin(cors::Any) // or use a specific origin: `AllowOrigin::exact("http://localhost:5173".parse().unwrap())`
        .allow_methods(cors::Any)
        .allow_headers(cors::Any);
    let extra_routes =
        OpenApiRouter::new()
            .routes(utoipa_axum::routes!(routes::test_json_nl_stream));
    let state = RequestState { storage: Arc::clone(&storage) };
    let addr = SocketAddr::from(([127,0,0,1], 8000));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let indexing_f = run::with_db(Arc::clone(&storage));
    let server_f = serve(state, addr, Some(extra_routes), Some(cors), shutdown_rx.clone());

    combine::futures(indexing_f, server_f, shutdown_tx).await;
}