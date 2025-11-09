use crate::storage::table_index_read::ReadOnlyIndexTable;
use crate::storage::table_writer_api::TableFactory;
use crate::{AppError, CacheKey, DbKey};
use lru::LruCache;
use redb::{Database, Key, MultimapTable, MultimapTableDefinition, Table, TableDefinition, WriteTransaction};
use std::fmt::Debug;
use std::num::NonZeroUsize;
use std::sync::Weak;

#[derive(Clone)]
pub struct IndexFactory<K: Key + 'static, V: Key + 'static> {
    pub(crate) name: String,
    pub(crate) pk_by_index_def: MultimapTableDefinition<'static, V, K>,
    pub(crate) index_by_pk_def: TableDefinition<'static, K, V>,
    pub(crate) lru_capacity: Option<usize>,
}


impl<K: Key + 'static, V: Key + 'static> Debug for IndexFactory<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexFactory").field("name", &self.name).finish()
    }
}

impl<K: Key + 'static, V: Key + 'static> IndexFactory<K, V> {
    pub fn new(name: &str, lru_capacity: usize, pk_by_index_def: MultimapTableDefinition<'static, V, K>, index_by_pk_def: TableDefinition<'static, K, V>) -> Self {
        let lru_cache_size_opt =
            if lru_capacity < 1 {
                None
            } else {
                Some(lru_capacity)
            };
        Self {
            name: name.to_string(),
            pk_by_index_def,
            index_by_pk_def,
            lru_capacity: lru_cache_size_opt
        }
    }
}

pub struct IndexTable<'txn, 'c, K: DbKey, V: CacheKey> {
    pub(crate) pk_by_index: MultimapTable<'txn, V, K>,
    pub(crate) index_by_pk: Table<'txn, K, V>,
    pub(crate) cache: Option<&'c mut LruCache<V::CK, K::Unit>>,
}

impl<'txn, 'c, K: DbKey, V: CacheKey> IndexTable<'txn, 'c, K, V> {
    pub fn new(write_tx: &'txn WriteTransaction, cache: Option<&'c mut LruCache<V::CK, K::Unit>>, pk_by_index_def: MultimapTableDefinition<'static, V, K>, index_by_pk_def: TableDefinition<'static, K, V>) -> Result<Self, AppError> {
        Ok(Self {
            pk_by_index: write_tx.open_multimap_table(pk_by_index_def)?,
            index_by_pk: write_tx.open_table(index_by_pk_def)?,
            cache
        })
    }
}

impl<K: DbKey, V: CacheKey> TableFactory<K, V> for IndexFactory<K, V> {
    type CacheCtx = Option<LruCache<V::CK, K::Unit>>;
    type Table<'txn, 'c> = IndexTable<'txn, 'c, K, V>;
    type ReadOnlyTable = ReadOnlyIndexTable<K, V>;

    fn name(&self) -> String {
        self.name.clone()
    }

    fn new_cache(&self) -> Self::CacheCtx {
        self.lru_capacity.map(|cap| LruCache::new(NonZeroUsize::new(cap).expect("lru_capacity for index must be > 0")))
    }

    fn open_for_write<'txn, 'c>(
        &self,
        tx: &'txn WriteTransaction,
        cache: &'c mut Self::CacheCtx,
    ) -> Result<Self::Table<'txn, 'c>, AppError> {
        IndexTable::new(
            tx,
            cache.as_mut(),
            self.pk_by_index_def,
            self.index_by_pk_def,
        )
    }

    fn open_for_read(&self, db_weak: &Weak<Database>) -> redb::Result<Self::ReadOnlyTable, AppError> {
        ReadOnlyIndexTable::new(
            db_weak,
            self.pk_by_index_def,
            self.index_by_pk_def,
        )
    }
}
