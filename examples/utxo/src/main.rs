use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use utxo::*;

#[tokio::main]
async fn main() {
    let dir = env::temp_dir().join("redbit");
    let db = 
        Arc::new(
            redb::Database::create(dir.join("my_db.redb"))
                .expect("Failed to create database")
        );
    db_demo::run(Arc::clone(&db)).expect("Db demo failed");
    serve(RequestState { db: Arc::clone(&db) }, SocketAddr::from(([127,0,0,1], 8000))).await
}