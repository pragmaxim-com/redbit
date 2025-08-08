use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use syncer::combine;
use tokio::sync::watch;
use tower_http::cors;
use utoipa_axum::router::OpenApiRouter;
use demo::*;

#[tokio::main]
async fn main() {
    let dir = env::temp_dir().join("redbit");
    if !dir.exists() {
        std::fs::create_dir_all(dir.clone()).unwrap();
    }
    let db = Arc::new(Database::create(dir.join("my_db.redb")).expect("Failed to create database"));

    let cors = cors::CorsLayer::new()
        .allow_origin(cors::Any) // or use a specific origin: `AllowOrigin::exact("http://localhost:5173".parse().unwrap())`
        .allow_methods(cors::Any)
        .allow_headers(cors::Any);
    let extra_routes =
        OpenApiRouter::new()
            .routes(utoipa_axum::routes!(routes::test_json_nl_stream));
    let state = RequestState { db: Arc::clone(&db) };
    let addr = SocketAddr::from(([127,0,0,1], 8000));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let indexing_f = run::with_db(Arc::clone(&db));
    let server_f = serve(state, addr, Some(extra_routes), Some(cors), shutdown_rx.clone());

    combine::futures(indexing_f, server_f, shutdown_tx).await;
}