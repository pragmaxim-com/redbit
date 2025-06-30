use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use redbit::utoipa_axum::router::OpenApiRouter;
use utxo::*;

#[tokio::main]
async fn main() {
    let dir = env::temp_dir().join("redbit");
    let db = 
        Arc::new(
            redb::Database::create(dir.join("my_db.redb"))
                .expect("Failed to create database")
        );
    demo::run(Arc::clone(&db)).expect("Db demo failed");
    let extra_routes = 
        OpenApiRouter::new()
            .routes(utoipa_axum::routes!(routes::foo_txs));
    let state = RequestState { db: Arc::clone(&db) };
    let addr = SocketAddr::from(([127,0,0,1], 8000));
    serve(state, addr, Some(extra_routes)).await
}