use std::env;
use std::sync::Arc;
use rand::random;
use redbit::redb::Database;
use redbit::Storage;
use crate::model_v1::Block;

pub fn empty_temp_storage(name: &str, db_cache_size_gb: u8) -> Arc<Storage> {
    let dir = env::temp_dir().join("redbit");
    if !dir.exists() {
        std::fs::create_dir_all(dir.clone()).unwrap();
    }
    let db_path = dir.join(format!("{}_{}.redb", name, random::<u64>()));
    let db = Database::builder().set_cache_size(db_cache_size_gb as usize * 1024 * 1024 * 1024).create(db_path).expect("Failed to create database");
    Arc::new(Storage::new(Arc::new(db)))
}

pub fn init_temp_storage(name: &str, db_cache_size_gb: u8) -> (Vec<Block>, Arc<Storage>) {
    let storage = empty_temp_storage(name, db_cache_size_gb);
    let write_tx = storage.begin_write().expect("Failed to begin write transaction");
    let blocks = Block::sample_many(3);
    Block::store_many(&write_tx, &blocks).expect("Failed to persist blocks");
    write_tx.commit().expect("Failed to commit transaction");
    (blocks, storage)
}
