use crate::*;
use futures_util::future::try_join_all;
use redb::{Database, DatabaseError, WriteTransaction};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::{env, fs};
use crate::table_writer::FlushFuture;

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
        let mut db_defs: Vec<DbDef> = Vec::new();
        for info in inventory::iter::<StructInfo> {
            db_defs.extend((info.db_defs)())
        }
        Self::init(db_dir, db_defs, db_cache_size_gb).await
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
        let mut db_defs = Vec::new();
        for info in inventory::iter::<StructInfo> {
            db_defs.extend((info.db_defs)())
        }
        let (_, storage) = Storage::init(db_path, db_defs, db_cache_size_gb).await?;
        Ok(storage)
    }

    pub async fn init(db_dir: PathBuf, db_defs: Vec<DbDef>, db_cache_size_gb: u8) -> redb::Result<(bool, Arc<Storage>), AppError> {
        let db_path = db_dir.join("plain.db");
        if !db_dir.exists() {
            fs::create_dir_all(db_dir.clone())?;
            let plain_db = Database::builder().set_cache_size(db_cache_size_gb as usize * 1024 * 1024 * 1024).create(db_path)?;
            let mut index_dbs = HashMap::new();
            for db_def in db_defs {
                let index_db_path = db_dir.join(format!("{}_index.db", db_def.name));
                let index_db = Database::builder().set_cache_size(db_def.cache * 1024 * 1024).create(index_db_path)?;
                index_dbs.insert(db_def.name, Arc::new(index_db));
            }
            Ok((true, Arc::new(Storage::new(Arc::new(plain_db), index_dbs))))
        } else {
            info!("Opening existing db at {:?}, it might take a while in case previous process was killed", db_path);
            let plain_db_task = {
                let path = db_path.to_path_buf();
                tokio::task::spawn_blocking(move || -> redb::Result<Database, DatabaseError> { Database::open(path) })
            };

            let index_tasks = db_defs.into_iter().map(|db_def| {
                let path = db_dir.join(format!("{}_index.db", db_def.name));
                tokio::task::spawn_blocking(move || -> redb::Result<(String, Arc<Database>), DatabaseError> {
                    let db = Database::open(path)?;
                    Ok((db_def.name, Arc::new(db)))
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
    fn flush_async(self) -> Result<Vec<FlushFuture>, AppError> where Self: Sized;
    fn flush(self) -> Result<Vec<()>, AppError> where Self: Sized {
        self.flush_async()?.into_iter().map(|f| f.wait()).collect::<Result<Vec<_>, _>>()
    }
}

pub trait ReadTxContext {
    fn begin_read_tx(storage: &Arc<Storage>) -> redb::Result<Self, AppError>
    where
        Self: Sized;
}
