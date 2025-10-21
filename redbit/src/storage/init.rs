use std::collections::HashMap;
use std::{env, fs};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use futures_util::future::try_join_all;
use redb::{Database, DatabaseError};
use crate::{error, info, AppError, DbDef, DbDefWithCache, StructInfo};
use crate::storage::cache;

#[derive(Clone)]
pub enum DbSetOwned {
    Single(Arc<Database>),
    Sharded(Vec<Arc<Database>>),
}

#[derive(Clone)]
pub enum DbSetWeak {
    Single(Weak<Database>),
    Sharded(Vec<Weak<Database>>),
}

impl DbSetOwned {
    pub fn new(name_index_dbs: Vec<(String, Option<usize>, Arc<Database>)>) -> HashMap<String, DbSetOwned> {
        let mut singles: HashMap<String, Arc<Database>> = HashMap::new();
        let mut shards:  HashMap<String, Vec<(usize, Arc<Database>)>> = HashMap::new();

        for (name, idx_opt, db) in name_index_dbs {
            match idx_opt {
                None => {
                    singles.insert(name, db);
                }
                Some(i) => {
                    shards.entry(name).or_default().push((i, db));
                }
            }
        }
        let mut out = HashMap::with_capacity(singles.len() + shards.len());
        for (name, db) in singles {
            out.insert(name, DbSetOwned::Single(db));
        }
        for (name, mut v) in shards {
            v.sort_by_key(|(i, _)| *i);
            out.insert(name, DbSetOwned::Sharded(v.into_iter().map(|(_, db)| db).collect()));
        }
        out
    }

    pub fn downgrade(&self) -> DbSetWeak {
        match self {
            DbSetOwned::Single(db) => DbSetWeak::Single(Arc::downgrade(db)),
            DbSetOwned::Sharded(dbs) => DbSetWeak::Sharded(dbs.iter().map(Arc::downgrade).collect()),
        }
    }

    pub fn assert_last_ref(&self, name: &str) {
        match self {
            DbSetOwned::Single(db) => {
                let sc = Arc::strong_count(db);
                if sc != 1 {
                    error!("Database {name} still has {sc} strong refs at shutdown");
                }
            },
            DbSetOwned::Sharded(dbs) => {
                let sc: usize = dbs.iter().map(|db|Arc::strong_count(db)).sum();
                if sc != dbs.len() {
                    error!("DbSet for {name} still has {sc} strong refs at shutdown instead of {}", dbs.len());
                }
            },
        }
    }
}

#[derive(Clone)]
pub struct Storage {
    pub index_dbs: HashMap<String, DbSetWeak>,
}

impl Storage {
    /// Fetch a *single* DB (clone the Weak). Errors if column missing or sharded.
    pub fn fetch_single_db(&self, name: &str) -> Result<Weak<Database>, AppError> {
        match self.index_dbs.get(name) {
            Some(DbSetWeak::Single(w)) => Ok(w.clone()),
            Some(DbSetWeak::Sharded(_)) => Err(AppError::Custom(format!(
                "column `{}`: expected single DB, found sharded", name
            ))),
            None => Err(AppError::Custom(format!("column `{}`: not found", name))),
        }
    }

    /// Fetch **all shards** (clone the Vec<Weak<_>>). Optionally enforce an expected shard count.
    pub fn fetch_sharded_dbs(&self, name: &str, expected: Option<usize>) -> Result<Vec<Weak<Database>>, AppError> {
        match self.index_dbs.get(name) {
            Some(DbSetWeak::Sharded(v)) => {
                let v = v.clone();
                if let Some(exp) = expected {
                    if v.len() != exp {
                        return Err(AppError::Custom(format!(
                            "column `{}`: shard count mismatch; expected {}, found {}",
                            name, exp, v.len()
                        )));
                    }
                }
                Ok(v)
            }
            Some(DbSetWeak::Single(db)) => Ok(vec![db.clone()]),
            None => Err(AppError::Custom(format!("column `{}`: not found", name))),
        }
    }
}

pub struct StorageOwner {
    pub index_dbs: HashMap<String, DbSetOwned>,
}

impl StorageOwner {
    pub fn new(index_dbs: HashMap<String, DbSetOwned>) -> Self {
        Self { index_dbs }
    }

    pub fn assert_last_refs(&self) {
        for (name, db_arc) in &self.index_dbs {
            db_arc.assert_last_ref(name);
        }
    }

    pub fn view(&self) -> Arc<Storage> {
        let mut m = HashMap::with_capacity(self.index_dbs.len());
        for (k, v) in &self.index_dbs {
            m.insert(k.clone(), v.downgrade());
        }
        Arc::new(Storage { index_dbs: m })
    }

    pub async fn build_storage(db_dir: PathBuf, db_cache_size_gb: u8) -> redb::Result<(bool, StorageOwner, Arc<Storage>), AppError> {
        let mut db_defs: Vec<DbDef> = Vec::new();
        for info in inventory::iter::<StructInfo> {
            db_defs.extend((info.db_defs)())
        }
        Self::init(db_dir, db_defs, db_cache_size_gb, true).await
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
        let (_, owner, view) = StorageOwner::init(db_path, db_defs, db_cache_size_gb, false).await?;
        Ok((owner, view))
    }

    fn log_name_with_cache_table(db_defs: &[DbDefWithCache]) -> Vec<String> {
        let name_width = db_defs.iter().map(|d| d.name.len()).max().unwrap_or(4); // at least "name"
        let mut lines = Vec::new();
        lines.push(format!("{:<name_width$}  {:>10}   {:>10}   {:>10}   {:>10}", "DB NAME", "weight", "size", "lru", "per shards", name_width = name_width));
        lines.extend(db_defs.iter().map(|d| {
            format!(
                "{:<name_width$}  {:>10}   {:>10}   {:>10}   {:>10}",
                d.name, d.db_cache_weight, d.db_cache_in_mb, d.lru_cache, d.shards, name_width = name_width,
            )
        }));
        lines
    }

    /// Create all DBs. `dbc.db_cache_in_mb` is already *per shard* for sharded, and
    /// per *single* DB for `shards == 0`. Reject `shards == 1`.
    fn build_owned_map_create(db_dir: &Path, defs: &[DbDefWithCache]) -> redb::Result<HashMap<String, DbSetOwned>, AppError> {
        let mut out = HashMap::with_capacity(defs.len());
        for dbc in defs {
            dbc.validate()?;
            match dbc.shards {
                1 => {
                    let db = Database::builder()
                        .set_cache_size(dbc.db_cache_in_mb)
                        .create(Self::db_file_path(db_dir, &dbc.name, None))?;
                    out.insert(dbc.name.clone(), DbSetOwned::Single(Arc::new(db)));
                }
                shards => {
                    let mut v = Vec::with_capacity(shards);
                    for shard_idx in 0..shards {
                        let db = Database::builder()
                            .set_cache_size(dbc.db_cache_in_mb)
                            .create(Self::db_file_path(db_dir, &dbc.name, Some(shard_idx)))?;
                        v.push(Arc::new(db));
                    }
                    out.insert(dbc.name.clone(), DbSetOwned::Sharded(v));
                }
            }
        }
        Ok(out)
    }

    /// Open all DBs in parallel. Reject `shards == 1`. Returns Single/Sharded depending on each definition.
    async fn build_owned_map_open(db_dir: &Path, defs: &[DbDefWithCache]) -> redb::Result<HashMap<String, DbSetOwned>, AppError> {
        for dbc in defs {
            dbc.validate()?;
        }
        let db_opening_tasks = defs.into_iter().flat_map(|dbc| {
            match dbc.shards {
                0 => {
                    let name = dbc.name.clone();
                    let path = Self::db_file_path(db_dir, &name, None);
                    vec![tokio::task::spawn_blocking(move ||
                        -> redb::Result<(String, Option<usize>, Arc<Database>), DatabaseError> {
                        let db = Database::open(path)?;
                        Ok((name, None, Arc::new(db)))
                    }
                    )]
                }
                n => (0..n).map(move |i| {
                    let name = dbc.name.clone();
                    let path = Self::db_file_path(db_dir, &name, Some(i));
                    tokio::task::spawn_blocking(move ||
                        -> redb::Result<(String, Option<usize>, Arc<Database>), DatabaseError> {
                        let db = Database::open(path)?;
                        Ok((name, Some(i), Arc::new(db)))
                    }
                    )
                }).collect::<Vec<_>>(),
            }
        });

        let opened = try_join_all(db_opening_tasks)
            .await?
            .into_iter()
            .collect::<redb::Result<Vec<(String, Option<usize>, Arc<Database>)>, DatabaseError>>()?;

        Ok(DbSetOwned::new(opened))
    }

    fn db_file_path(dir: &Path, name: &str, shard_idx: Option<usize>) -> PathBuf {
        match shard_idx {
            Some(i) => dir.join(format!("{}-{}.db", name, i)),
            None    => dir.join(format!("{}.db",    name)),
        }
    }

    pub async fn init(db_dir: PathBuf, db_defs: Vec<DbDef>, total_cache_size_gb: u8, log_info: bool) -> redb::Result<(bool, StorageOwner, Arc<Storage>), AppError> {
        // allocator now outputs per-shard cache in MB + asserts shards >= 2
        let defs_with_cache: Vec<DbDefWithCache> = cache::allocate_cache_mb(&db_defs, (total_cache_size_gb as u64) * 1024);

        let result =
            if !db_dir.exists() {
                fs::create_dir_all(&db_dir)?;
                info!("Creating dbs at {:?} with total cache size {} GB", db_dir, total_cache_size_gb);
                let index_dbs = Self::build_owned_map_create(&db_dir, &defs_with_cache)?;
                let owner = StorageOwner::new(index_dbs);
                let view = owner.view();
                Ok((true, owner, view))
            } else {
                info!(
                    "Opening existing dbs at {:?} with total cache size {} GB, it might take a while in case previous process was killed",
                    db_dir, total_cache_size_gb
                );
                let index_dbs = Self::build_owned_map_open(&db_dir, &defs_with_cache).await?;
                let owner = StorageOwner::new(index_dbs);
                let view = owner.view();
                Ok((false, owner, view))
            };
        if log_info {
            info!("DB report:\n{}", Self::log_name_with_cache_table(&defs_with_cache).join("\n"));
        }
        result
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use std::sync::Arc;
    use crate::storage::init::{DbSetWeak, Storage, StorageOwner};

    fn count_weak_upgrades(storage: &Arc<Storage>) -> (usize, usize) {
        let mut total = 0usize;
        let mut alive = 0usize;

        for group in storage.index_dbs.values() {
            match group {
                DbSetWeak::Single(w) => {
                    total += 1;
                    if w.upgrade().is_some() { alive += 1; }
                }
                DbSetWeak::Sharded(ws) => {
                    total += ws.len();
                    alive += ws.iter().filter(|w| w.upgrade().is_some()).count();
                }
            }
        }
        (total, alive)
    }

    #[tokio::test]
    async fn test_storage_weak_owner_drop() {
        // NOTE: shards must be >= 2 now
        let (owner, storage) = StorageOwner::temp("weak_drop_test", 2, true)
            .await
            .expect("temp storage");

        // While owner is alive, Weak<Database> should upgrade (if any exist)
        let (total_before, alive_before) = count_weak_upgrades(&storage);

        // If the fixture created no DBs (empty schema), gracefully skip assertions.
        if total_before == 0 {
            eprintln!("no databases in temp storage; skipping drop assertions");
            return;
        }

        assert_eq!(alive_before, total_before, "all dbs should be alive before drop");

        // Drop owner -> all Weak must fail to upgrade
        drop(owner);

        let (_total_after, alive_after) = count_weak_upgrades(&storage);
        assert_eq!(alive_after, 0, "all dbs must be dropped when owner is dropped");
    }

    #[tokio::test]
    async fn test_storage_groups_have_consistent_shape() {
        let (_owner, storage) = StorageOwner::temp("shape_test", 1, true)
            .await
            .expect("temp storage");

        // Just verify we can iterate and see at least one weak in each group.
        for (name, group) in &storage.index_dbs {
            let weak_count = match group {
                DbSetWeak::Single(w) => if w.upgrade().is_some() { 1 } else { 0 },
                DbSetWeak::Sharded(ws) => ws.len(),
            };
            assert!(weak_count > 0, "column `{name}` must hold at least one weak ref");
        }
    }
}
