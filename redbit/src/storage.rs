use crate::table_writer::FlushFuture;
use crate::*;
use futures_util::future::try_join_all;
use redb::{Database, DatabaseError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::{env, fs};

#[derive(Clone)]
pub struct Storage {
    pub index_dbs: HashMap<String, Arc<Database>>,
    pub total_cache_size_gb: u8
}

impl Storage {
    pub fn new(index_dbs: HashMap<String, Arc<Database>>, total_cache_size_gb: u8) -> Self {
        Self { index_dbs, total_cache_size_gb }
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

    pub async fn init(db_dir: PathBuf, db_defs: Vec<DbDef>, total_cache_size_gb: u8) -> redb::Result<(bool, Arc<Storage>), AppError> {
        if !db_dir.exists() {
            fs::create_dir_all(db_dir.clone())?;
            let mut index_dbs = HashMap::new();
            let total_cache_bytes: u64 = (total_cache_size_gb as u64) * 1024 * 1024 * 1024;
            let db_defs_with_cache: Vec<DbDefWithCache> = cache::allocate_cache_mb(&db_defs, total_cache_bytes);

            for dbc in db_defs_with_cache {
                let index_db_path = db_dir.join(format!("{}.db", dbc.name));
                info!("Creating db at {:?} with cache size {} MB", index_db_path, dbc.cache_in_mb);
                let index_db = Database::builder().set_cache_size(dbc.cache_in_mb).create(index_db_path)?;
                index_dbs.insert(dbc.name, Arc::new(index_db));
            }

            Ok((true, Arc::new(Storage::new(index_dbs, total_cache_size_gb))))
        } else {
            info!("Opening existing dbs at {:?}, it might take a while in case previous process was killed", db_dir);
            let index_tasks = db_defs.into_iter().map(|db_def| {
                let path = db_dir.join(format!("{}.db", db_def.name));
                tokio::task::spawn_blocking(move || -> redb::Result<(String, Arc<Database>), DatabaseError> {
                    let db = Database::open(path)?;
                    Ok((db_def.name, Arc::new(db)))
                })
            });

            let index_dbs = try_join_all(index_tasks)
                .await?
                .into_iter()
                .collect::<redb::Result<HashMap<String, Arc<Database>>, DatabaseError>>()?;

            Ok((false, Arc::new(Storage::new(index_dbs, total_cache_size_gb))))
        }
    }
}

pub trait WriteTxContext {
    fn begin_write_tx(storage: &Arc<Storage>) -> redb::Result<Self, AppError>
    where
        Self: Sized;
    fn commit_all_async(self) -> Result<Vec<FlushFuture>, AppError> where Self: Sized;
    fn commit_all(self) -> Result<Vec<()>, AppError> where Self: Sized {
        self.commit_all_async()?.into_iter().map(|f| f.wait()).collect::<Result<Vec<_>, _>>()
    }
    fn two_phase_commit(self) -> Result<(), AppError>;
}

pub trait ReadTxContext {
    fn begin_read_tx(storage: &Arc<Storage>) -> redb::Result<Self, AppError>
    where
        Self: Sized;
}
