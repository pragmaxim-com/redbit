use crate::storage::cache;
use crate::{error, info, AppError, DbDef, DbDefWithCache, StructInfo};
use futures_util::future::try_join_all;
use redb::{Database, DatabaseError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::{env, fs};

#[derive(Clone)]
pub struct DbSetOwned(Vec<Arc<Database>>);

#[derive(Clone)]
pub struct DbSetWeak(Vec<Weak<Database>>);

impl DbSetOwned {
    pub fn new(name_index_dbs: Vec<(String, usize, Arc<Database>)>) -> HashMap<String, DbSetOwned> {
        let mut shards:  HashMap<String, Vec<(usize, Arc<Database>)>> = HashMap::new();
        for (name, idx, db) in name_index_dbs {
            shards.entry(name).or_default().push((idx, db));
        }
        let mut out = HashMap::with_capacity(shards.len());
        for (name, mut v) in shards {
            v.sort_by_key(|(i, _)| *i);
            out.insert(name, DbSetOwned(v.into_iter().map(|(_, db)| db).collect()));
        }
        out
    }

    pub fn downgrade(&self) -> DbSetWeak {
        DbSetWeak(self.0.iter().map(Arc::downgrade).collect())
    }

    pub fn assert_last_ref(&self, name: &str) {
        let sc: usize = self.0.iter().map(|db|Arc::strong_count(db)).sum();
        if sc != self.0.len() {
            error!("DbSet for {name} still has {sc} strong refs at shutdown instead of {}", self.0.len());
        }
    }
}

#[derive(Clone)]
pub struct Storage {
    pub index_dbs: HashMap<String, DbSetWeak>,
}

impl Storage {
    /// Fetch **all shards** (clone the Vec<Weak<_>>). Optionally enforce an expected shard count.
    pub fn fetch_dbs(&self, name: &str) -> Result<Vec<Weak<Database>>, AppError> {
        match self.index_dbs.get(name) {
            Some(DbSetWeak(v)) => Ok(v.clone()),
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

    fn build_owned_map_create(db_dir: &Path, defs: &[DbDefWithCache]) -> redb::Result<HashMap<String, DbSetOwned>, AppError> {
        let mut out = HashMap::with_capacity(defs.len());
        for dbc in defs {
            dbc.validate()?;
            let mut v = Vec::with_capacity(dbc.shards);
            for shard_idx in 0..dbc.shards {
                let suffix = match dbc.shards {
                    1 => None,
                    _ => Some(shard_idx),
                };
                let db = Database::builder()
                    .set_cache_size(dbc.db_cache_in_mb)
                    .create(Self::db_file_path(db_dir, &dbc.name, suffix))?;
                v.push(Arc::new(db));
            }
            out.insert(dbc.name.clone(), DbSetOwned(v));
        }
        Ok(out)
    }

    async fn build_owned_map_open(db_dir: &Path, defs: &[DbDefWithCache]) -> redb::Result<HashMap<String, DbSetOwned>, AppError> {
        for dbc in defs {
            dbc.validate()?;
        }
        let db_opening_tasks = defs.into_iter().flat_map(|dbc| {
            (0..dbc.shards).map(move |idx| {
                let name = dbc.name.clone();
                let suffix = match dbc.shards {
                    1 => None,
                    _ => Some(idx),
                };
                let path = Self::db_file_path(db_dir, &name, suffix);
                tokio::task::spawn_blocking(move ||
                    -> redb::Result<(String, usize, Arc<Database>), DatabaseError> {
                        let db = Database::open(path)?;
                        Ok((name, idx, Arc::new(db)))
                    }
                )
            }).collect::<Vec<_>>()
        });

        let opened = try_join_all(db_opening_tasks)
            .await?
            .into_iter()
            .collect::<redb::Result<Vec<(String, usize, Arc<Database>)>, DatabaseError>>()?;

        Ok(DbSetOwned::new(opened))
    }

    fn db_file_path(dir: &Path, name: &str, shard_idx: Option<usize>) -> PathBuf {
        match shard_idx {
            Some(i) => dir.join(format!("{}-{}.db", name, i)),
            None    => dir.join(format!("{}.db",    name)),
        }
    }

    pub async fn init(db_dir: PathBuf, db_defs: Vec<DbDef>, total_cache_size_gb: u8, log_info: bool) -> redb::Result<(bool, StorageOwner, Arc<Storage>), AppError> {
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
    use crate::storage::init::{Storage, StorageOwner};
    use std::sync::Arc;

    fn count_weak_upgrades(storage: &Arc<Storage>) -> (usize, usize) {
        let mut total = 0usize;
        let mut alive = 0usize;

        for group in storage.index_dbs.values() {
            total += group.0.len();
            alive += group.0.iter().filter(|w| w.upgrade().is_some()).count();
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

        for (name, group) in &storage.index_dbs {
            let weak_count = group.0.len();
            assert!(weak_count > 0, "column `{name}` must hold at least one weak ref");
        }
    }
}
