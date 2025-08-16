use std::{env, fs};
use std::hash::Hash;
use std::sync::Arc;
use redb::{Database, WriteTransaction, TableDefinition, Table, Key, Value, CommitError, TransactionError, TableError, MultimapTableDefinition, MultimapTable, ReadTransaction, ReadOnlyTable, ReadOnlyMultimapTable};
use crate::cache::Caches;
use crate::*;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Storage {
    pub db: Arc<Database>,
    caches: Arc<Caches>, // your existing cache holder
}

impl Storage {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db: Arc::clone(&db), caches: Arc::new(Caches::default()) }
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
        let storage = Storage::init(db_path, db_cache_size_gb)?;
        Ok(storage)
    }

    pub fn init(db_dir: PathBuf, db_cache_size_gb: u8) -> redb::Result<Arc<Storage>, AppError> {
        let db_path = db_dir.join("chain_syncer.db");
        if !db_dir.exists() {
            fs::create_dir_all(db_dir.clone())?;
            let db = Database::builder().set_cache_size(db_cache_size_gb as usize * 1024 * 1024 * 1024).create(db_path)?;
            Ok(Arc::new(Storage::new(Arc::new(db))))
        } else {
            info!("Opening existing db at {:?}, it might take a while in case previous process was killed", db_path);
            let db = Database::open(db_path)?;
            Ok(Arc::new(Storage::new(Arc::new(db))))
        }
    }

    pub fn begin_read(&self) -> redb::Result<StorageReadTx, TransactionError> {
        use redb::ReadableDatabase;
        let tx = self.db.begin_read()?;
        Ok(StorageReadTx { tx })
    }

    pub fn begin_write(&self) -> redb::Result<StorageWriteTx, TransactionError> {
        let tx = self.db.begin_write()?;
        Ok(StorageWriteTx { tx, caches: Arc::clone(&self.caches) })
    }
}

pub struct StorageReadTx {
    tx: ReadTransaction,
}

impl StorageReadTx {
    pub fn open_table<K, V>(&self, def: TableDefinition<K, V>) -> redb::Result<ReadOnlyTable<K, V>, TableError>
    where
        K: Key + 'static,
        V: Value + 'static,
    {
        self.tx.open_table(def)
    }

    pub fn open_multimap_table<K, V>(
        &self,
        def: MultimapTableDefinition<K, V>,
    ) -> redb::Result<ReadOnlyMultimapTable<K, V>, TableError>
    where
        K: Key + 'static,
        V: Key + 'static,
    {
        self.tx.open_multimap_table(def)
    }
}

pub struct StorageWriteTx {
    tx: WriteTransaction,
    pub(crate) caches: Arc<Caches>,
}

impl StorageWriteTx {
    pub fn commit(self) -> redb::Result<(), CommitError> {
        self.tx.commit()
    }

    pub fn open_table<'txn, K, V>(&'txn self, def: TableDefinition<K, V>) -> redb::Result<Table<'txn, K, V>, TableError>
    where
        K: Key + 'static,
        V: Value + 'static,
    {
        self.tx.open_table(def)
    }

    pub fn open_multimap_table<'txn, K, V>(
        &'txn self,
        def: MultimapTableDefinition<K, V>,
    ) -> redb::Result<MultimapTable<'txn, K, V>, TableError>
    where
        K: Key + 'static,
        V: Key + 'static,
    {
        self.tx.open_multimap_table(def)
    }

    pub fn cache_get<K, V>(&self, def: &'static CacheDef<K, V>, k: &K) -> Option<V>
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let cache = self.caches.get_cache(def);
        let mut c = cache.lock().unwrap();
        c.get(k).cloned()
    }

    pub fn cache_put<K, V>(&self, def: &'static CacheDef<K, V>, k: K, v: V)
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let cache = self.caches.get_cache(def);
        let mut c = cache.lock().unwrap();
        let _ = c.put(k, v);
    }

    pub fn cache_remove<K, V>(&self, def: &'static CacheDef<K, V>, k: &K) -> Option<V>
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let cache = self.caches.get_cache(def);
        let mut c = cache.lock().unwrap();
        c.pop(k)
    }
}
