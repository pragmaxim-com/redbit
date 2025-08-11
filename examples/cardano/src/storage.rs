use crate::model_v1::Block;
use redb::Database;
use redbit::*;
use std::path::PathBuf;

pub fn get_storage(db_dir: PathBuf, db_cache_size_gb: u8) -> redb::Result<Arc<Storage>, AppError> {
    if !db_dir.exists() {
        std::fs::create_dir_all(db_dir.clone()).map_err(|e| AppError::Internal(format!("Failed to create database directory: {}", e)))?;
        let db = Database::builder().set_cache_size(db_cache_size_gb as usize * 1024 * 1024 * 1024).create(db_dir.join("chain_syncer.db"))?;
        let storage = Arc::new(Storage::new(Arc::new(db)));
        let sample_block = Block::sample();
        Block::store_and_commit(Arc::clone(&storage), &sample_block)?;
        Block::delete_and_commit(Arc::clone(&storage), &sample_block.height)?;
        Ok(Arc::clone(&storage))
    } else {
        let db = Database::open(db_dir.join("chain_syncer.db"))?;
        Ok(Arc::new(Storage::new(Arc::new(db))))
    }
}
