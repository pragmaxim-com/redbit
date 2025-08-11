use std::env;
use rand::random;
use redb::Database;
use crate::*;

pub fn open_or_create_db(name: &str) -> Database {
    let dir = env::temp_dir().join("redbit");
    if !dir.exists() {
        std::fs::create_dir_all(dir.clone()).unwrap();
    }
    let db_path = dir.join(format!("{}.redb", name));
    Database::create(db_path).expect("Failed to create database")
}

pub fn empty_temp_storage(name: &str) -> Arc<Storage> {
    let dir = env::temp_dir().join("redbit");
    if !dir.exists() {
        std::fs::create_dir_all(dir.clone()).unwrap();
    }
    let db_path = dir.join(format!("{}_{}.redb", name, random::<u64>()));
    let db = Database::create(db_path).expect("Failed to create database");
    Arc::new(Storage::new(Arc::new(db)))
}

pub fn init_temp_storage(name: &str) -> (Vec<Block>, Arc<Storage>) {
    let storage = empty_temp_storage(name);
    let write_tx = storage.begin_write().expect("Failed to begin write transaction");
    let blocks = Block::sample_many(3);
    Block::store_many(&write_tx, &blocks).expect("Failed to persist blocks");
    write_tx.commit().expect("Failed to commit transaction");
    (blocks, storage)
}
