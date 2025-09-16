use crate::*;
use futures_util::future::try_join_all;
use redb::{Database, DatabaseError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::{env, fs};
use table_writer::FlushFuture;

pub mod table_dict_read;
pub mod table_dict_write;
pub mod table_index_read;
pub mod table_index_write;
pub mod table_plain_write;
pub mod table_writer;
pub mod cache;

#[derive(Clone)]
pub struct Storage {
    pub index_dbs: HashMap<String, Arc<Database>>,
}

impl Storage {
    pub fn new(index_dbs: HashMap<String, Arc<Database>>) -> Self {
        Self { index_dbs }
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
        let db_defs_with_cache: Vec<DbDefWithCache> = cache::allocate_cache_mb(&db_defs, (total_cache_size_gb as u64) * 1024);
        let width = db_defs_with_cache.iter().map(|d| d.name.len()).max().unwrap_or(0);
        let db_name_with_cache_list = db_defs_with_cache
            .iter()
            .map(|d| format!("{:<width$} {} MB", d.name, d.cache_in_mb, width = width))
            .collect::<Vec<_>>()
            .join("\n");

        if !db_dir.exists() {
            fs::create_dir_all(db_dir.clone())?;
            let mut index_dbs = HashMap::new();
            info!("Creating dbs at {:?} with total cache size {} GB:\n{}", db_dir, total_cache_size_gb, db_name_with_cache_list);
            for dbc in db_defs_with_cache {
                let index_db_path = db_dir.join(format!("{}.db", dbc.name));
                let index_db = Database::builder().set_cache_size(dbc.cache_in_mb).create(index_db_path)?;
                index_dbs.insert(dbc.name, Arc::new(index_db));
            }

            Ok((true, Arc::new(Storage::new(index_dbs))))
        } else {
            info!(
                "Opening existing dbs at {:?} with total cache size {} GB:\n{}, it might take a while in case previous process was killed",
                db_dir,
                total_cache_size_gb,
                db_name_with_cache_list
            );
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

            Ok((false, Arc::new(Storage::new(index_dbs))))
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
