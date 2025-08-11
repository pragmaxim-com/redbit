use std::hash::Hash;
use std::sync::Arc;
use redb::{Database, WriteTransaction, TableDefinition, Table, Key, Value, CommitError, TransactionError, TableError, MultimapTableDefinition, MultimapTable, ReadTransaction, ReadOnlyTable, ReadOnlyMultimapTable};
use crate::cache::Caches;
use crate::CacheDef;

#[derive(Clone)]
pub struct Storage {
    pub db: Arc<Database>,
    caches: Arc<Caches>, // your existing cache holder
}

impl Storage {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db: Arc::clone(&db), caches: Arc::new(Caches::default()) }
    }

    pub fn begin_read(&self) -> redb::Result<StorageReadTx, TransactionError> {
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

    // Forwarders keep the method-level 'txn lifetime just like redb
    pub fn open_table<K, V>(&self, def: TableDefinition<K, V>) -> redb::Result<Table<K, V>, TableError>
    where
        K: Key + 'static,
        V: Value + 'static,
    {
        self.tx.open_table(def)
    }

    pub fn open_multimap_table<K, V>(
        &self,
        def: MultimapTableDefinition<K, V>,
    ) -> redb::Result<MultimapTable<K, V>, TableError>
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
        let cache = self.caches.ensure_cache(def);
        let mut c = cache.lock().unwrap();
        c.get(k).cloned()
    }

    pub fn cache_put<K, V>(&self, def: &'static CacheDef<K, V>, k: K, v: V)
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let cache = self.caches.ensure_cache(def);
        let mut c = cache.lock().unwrap();
        let _ = c.put(k, v);
    }

    pub fn cache_remove<K, V>(&self, def: &'static CacheDef<K, V>, k: &K) -> Option<V>
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let cache = self.caches.ensure_cache(def);
        let mut c = cache.lock().unwrap();
        c.pop(k)
    }
}
