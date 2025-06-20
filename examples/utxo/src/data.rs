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

pub fn empty_temp_db(name: &str) -> Database {
    let dir = env::temp_dir().join("redbit");
    if !dir.exists() {
        std::fs::create_dir_all(dir.clone()).unwrap();
    }
    let db_path = dir.join(format!("{}_{}.redb", name, random::<u64>()));
    Database::create(db_path).expect("Failed to create database")
}

pub fn init_temp_db(name: &str) -> (Vec<Block>, Database) {
    let db = empty_temp_db(name);
    let blocks = get_blocks(3);
    let write_tx = db.begin_write().expect("Failed to begin write transaction");
    Block::store_many(&write_tx, &blocks).expect("Failed to persist blocks");
    write_tx.commit().expect("Failed to commit transaction");
    (blocks, db)
}

pub fn get_blocks(block_count: Height) -> Vec<Block> {
    (0..block_count)
        .map(|height| Block::sample_with(&BlockPointer(height)))
        .collect::<Vec<_>>()
}
