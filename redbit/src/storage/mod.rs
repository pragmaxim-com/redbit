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
    pub index_dbs: HashMap<String, Weak<Database>>,
}

pub struct StorageOwner {
    pub index_dbs: HashMap<String, Arc<Database>>,
}

impl StorageOwner {
    pub fn view(&self) -> Arc<Storage> {
        let mut weak_map = HashMap::with_capacity(self.index_dbs.len());
        for (k, v) in &self.index_dbs {
            weak_map.insert(k.clone(), Arc::downgrade(v));
        }
        Arc::new(Storage { index_dbs: weak_map })
    }
}

impl StorageOwner {
    pub fn new(index_dbs: HashMap<String, Arc<Database>>) -> Self {
        Self { index_dbs }
    }

    pub async fn build_storage(db_dir: PathBuf, db_cache_size_gb: u8) -> redb::Result<(bool, StorageOwner, Arc<Storage>), AppError> {
        let mut db_defs: Vec<DbDef> = Vec::new();
        for info in inventory::iter::<StructInfo> {
            db_defs.extend((info.db_defs)())
        }
        Self::init(db_dir, db_defs, db_cache_size_gb).await
    }

    pub async fn temp(name: &str, db_cache_size_gb: u8, random: bool) -> redb::Result<(StorageOwner, Arc<Storage>), AppError> {
        let db_name = if random { format!("{}_{}", name, rand::random::<u64>()) } else { name.to_string() };
        let db_path = env::temp_dir().join(format!("{}/{}", "redbit", db_name));
        if random && db_path.exists() {
            fs::remove_dir_all(&db_path)?;
        }
        let mut db_defs = Vec::new();
        for info in inventory::iter::<StructInfo> {
            db_defs.extend((info.db_defs)())
        }
        let (_, owner, view) = StorageOwner::init(db_path, db_defs, db_cache_size_gb).await?;
        Ok((owner, view))
    }

    fn db_name_with_cache_table(db_defs: &[DbDefWithCache]) -> Vec<String> {
        let name_width = db_defs.iter().map(|d| d.name.len()).max().unwrap_or(4); // at least "name"
        let mut lines = Vec::new();
        lines.push(format!("{:<name_width$}  {:>10}   {:>10}   {:>10}", "DB NAME", "weight", "size", "lru", name_width = name_width));
        lines.extend(db_defs.iter().map(|d| {
            format!(
                "{:<name_width$}  {:>10}   {:>10}   {:>10}",
                d.name, d.db_cache_weight, d.db_cache_in_mb, d.lru_cache,
                name_width = name_width,
            )
        }));
        lines
    }

    pub async fn init(db_dir: PathBuf, db_defs: Vec<DbDef>, total_cache_size_gb: u8) -> redb::Result<(bool, StorageOwner, Arc<Storage>), AppError> {
        let db_defs_with_cache: Vec<DbDefWithCache> = cache::allocate_cache_mb(&db_defs, (total_cache_size_gb as u64) * 1024);
        let db_name_with_cache_table = Self::db_name_with_cache_table(&db_defs_with_cache).join("\n");
        if !db_dir.exists() {
            fs::create_dir_all(db_dir.clone())?;
            let mut index_dbs = HashMap::new();
            info!("Creating dbs at {:?} with total cache size {} GB:\n{}", db_dir, total_cache_size_gb, db_name_with_cache_table);
            for dbc in db_defs_with_cache {
                let index_db_path = db_dir.join(format!("{}.db", dbc.name));
                let index_db = Database::builder().set_cache_size(dbc.db_cache_in_mb).create(index_db_path)?;
                index_dbs.insert(dbc.name, Arc::new(index_db));
            }
            let owner = StorageOwner::new(index_dbs);
            let view = owner.view();
            Ok((true, owner, view))
        } else {
            info!(
                "Opening existing dbs at {:?} with total cache size {} GB, it might take a while in case previous process was killed\n{}",
                db_dir, total_cache_size_gb, db_name_with_cache_table
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

            let owner = StorageOwner::new(index_dbs);
            let view = owner.view();
            Ok((false, owner, view))
        }
    }
}

pub trait WriteTxContext {
    fn new_write_ctx(storage: &Arc<Storage>) -> redb::Result<Self, AppError> where Self: Sized;
    fn begin_writing(&self) -> redb::Result<(), AppError>;
    fn stop_writing(self) -> redb::Result<(), AppError> where Self: Sized;
    fn commit_ctx_async(&self) -> Result<Vec<FlushFuture>, AppError>;
    fn two_phase_commit(&self) -> Result<(), AppError>;

    fn begin_write_ctx(storage: &Arc<Storage>) -> redb::Result<Self, AppError> where Self: Sized {
        let ctx = Self::new_write_ctx(storage)?;
        let _ = ctx.begin_writing()?;
        Ok(ctx)
    }
    fn commit_and_close_ctx(self) -> Result<(), AppError> where Self: Sized {
        let _ = self.commit_ctx_async()?.into_iter().map(|f| f.wait()).collect::<Result<Vec<_>, _>>();
        self.stop_writing()
    }
    fn two_phase_commit_and_close(self) -> Result<(), AppError> where Self: Sized {
        self.two_phase_commit()?;
        self.stop_writing()
    }
}

pub trait ReadTxContext {
    fn begin_read_ctx(storage: &Arc<Storage>) -> redb::Result<Self, AppError>
    where
        Self: Sized;
}
