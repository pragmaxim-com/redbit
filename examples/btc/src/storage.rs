use crate::model::Block;
use redb::Database;
use redbit::*;
use std::path::PathBuf;

pub fn get_db(db_dir: PathBuf, db_cache_size_gb: u8) -> redb::Result<Database, AppError> {
    if !db_dir.exists() {
        std::fs::create_dir_all(db_dir.clone()).map_err(|e| AppError::Internal(format!("Failed to create database directory: {}", e)))?;
        let db = Database::builder().set_cache_size(db_cache_size_gb as usize * 1024 * 1024 * 1024).create(db_dir.join("chain_syncer.db"))?;
        let sample_block = Block::sample();
        Block::store_and_commit(&db, &sample_block)?;
        Block::delete_and_commit(&db, &sample_block.id)?;
        Ok(db)
    } else {
        Database::open(db_dir.join("chain_syncer.db")).map_err(|e| e.into())
    }
}
