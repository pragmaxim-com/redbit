use crate::*;
use futures_util::future::try_join_all;
use redb::{Database, DatabaseError, WriteTransaction};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::{env, fs};

#[derive(Clone)]
pub struct Storage {
    pub plain_db: Arc<Database>,
    pub index_dbs: HashMap<String, Arc<Database>>,
}

impl Storage {
    pub fn new(plain_db: Arc<Database>, index_dbs: HashMap<String, Arc<Database>>) -> Self {
        Self { plain_db, index_dbs }
    }

    pub async fn build_storage(db_dir: PathBuf, db_cache_size_gb: u8) -> redb::Result<(bool, Arc<Storage>), AppError> {
        let mut db_names: Vec<String> = Vec::new();
        for info in inventory::iter::<StructInfo> {
            db_names.extend((info.db_names)())
        }
        Self::init(db_dir, db_names, db_cache_size_gb).await
    }


    pub async fn temp(name: &str, db_cache_size_gb: u8, random: bool) -> redb::Result<Arc<Storage>, AppError> {
        let db_name = if random {
            format!("{}_{}", name, rand::random::<u64>())
        } else {
            name.to_string()
        };
        let db_path = env::temp_dir().join(format!("{}/{}", "redbit", db_name));
        if random && db_path.exists() {
            fs::remove_dir_all(&db_path)?;
        }
        let mut db_names = Vec::new();
        for info in inventory::iter::<StructInfo> {
            db_names.extend((info.db_names)())
        }
        let (_, storage) = Storage::init(db_path, db_names, db_cache_size_gb).await?;
        Ok(storage)
    }

    pub async fn init(db_dir: PathBuf, db_names: Vec<String>, db_cache_size_gb: u8) -> redb::Result<(bool, Arc<Storage>), AppError> {
        let total_dbs = db_names.len() + 1;
        let per_db_cache_size_gb = if db_cache_size_gb == 0 { 1 } else { db_cache_size_gb / (total_dbs as u8) };
        let db_path = db_dir.join("plain.db");
        if !db_dir.exists() {
            fs::create_dir_all(db_dir.clone())?;
            let plain_db = Database::builder().set_cache_size(per_db_cache_size_gb as usize * 1024 * 1024 * 1024).create(db_path)?;
            let mut index_dbs = HashMap::new();
            for db_name in db_names {
                let index_db_path = db_dir.join(format!("{}_index.db", db_name));
                let index_db = Database::builder().set_cache_size(per_db_cache_size_gb as usize * 1024 * 1024 * 1024).create(index_db_path)?;
                index_dbs.insert(db_name.to_string(), Arc::new(index_db));
            }
            Ok((true, Arc::new(Storage::new(Arc::new(plain_db), index_dbs))))
        } else {
            info!("Opening existing db at {:?}, it might take a while in case previous process was killed", db_path);
            let plain_db_task = {
                let path = db_path.to_path_buf();
                tokio::task::spawn_blocking(move || -> redb::Result<Database, DatabaseError> { Database::open(path) })
            };

            let index_tasks = db_names.into_iter().map(|db_name| {
                let name = db_name.to_string();
                let path = db_dir.join(format!("{}_index.db", db_name));
                tokio::task::spawn_blocking(move || -> redb::Result<(String, Arc<Database>), DatabaseError> {
                    let db = Database::open(path)?;
                    Ok((name, Arc::new(db)))
                })
            });

            let plain_db = plain_db_task.await??;

            let index_dbs = try_join_all(index_tasks)
                .await?
                .into_iter()
                .collect::<redb::Result<HashMap<String, Arc<Database>>, DatabaseError>>()?;

            Ok((false, Arc::new(Storage::new(Arc::new(plain_db), index_dbs))))
        }
    }

}

pub trait WriteTxContext<'txn> {
    fn begin_write_tx(plain_tx: &'txn WriteTransaction, index_dbs: &HashMap<String, Arc<Database>>) -> redb::Result<Self, AppError>
    where
        Self: Sized;
    fn flush(self) -> Result<(), AppError>;
}

pub trait ReadTxContext {
    fn begin_read_tx(storage: &Arc<Storage>) -> redb::Result<Self, AppError>
    where
        Self: Sized;
}
