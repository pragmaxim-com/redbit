use crate::*;
use redb::{Database, TableError, WriteTransaction};
use std::path::PathBuf;
use std::sync::Arc;
use std::{env, fs};

#[derive(Clone)]
pub struct Storage {
    pub db: Arc<Database>,
}

impl Storage {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db: Arc::clone(&db) }
    }

    pub fn temp(name: &str, db_cache_size_gb: u8, random: bool) -> redb::Result<Arc<Storage>, AppError> {
        let db_name = if random {
            format!("{}_{}", name, rand::random::<u64>())
        } else {
            name.to_string()
        };
        let db_path = env::temp_dir().join(format!("{}/{}", "redbit", db_name));
        if random && db_path.exists() {
            fs::remove_dir_all(&db_path)?;
        }
        let (_, storage) = Storage::init(db_path, db_cache_size_gb)?;
        Ok(storage)
    }

    pub fn init(db_dir: PathBuf, db_cache_size_gb: u8) -> redb::Result<(bool, Arc<Storage>), AppError> {
        let db_path = db_dir.join("chain.db");
        if !db_dir.exists() {
            fs::create_dir_all(db_dir.clone())?;
            let db = Database::builder().set_cache_size(db_cache_size_gb as usize * 1024 * 1024 * 1024).create(db_path)?;
            Ok((true, Arc::new(Storage::new(Arc::new(db)))))
        } else {
            info!("Opening existing db at {:?}, it might take a while in case previous process was killed", db_path);
            let db = Database::open(db_path)?;
            Ok((false, Arc::new(Storage::new(Arc::new(db)))))
        }
    }

}

pub trait WriteTxContext<'txn> {
    fn begin_write_tx(tx: &'txn WriteTransaction) -> redb::Result<Self, TableError>
    where
        Self: Sized;
}

pub trait ReadTxContext {
    fn begin_read_tx(tx: &ReadTransaction) -> redb::Result<Self, TableError>
    where
        Self: Sized;
}
