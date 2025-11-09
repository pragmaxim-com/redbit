use crate::storage::table_dict_read::ReadOnlyDictTable;
use crate::storage::table_writer_api::TableFactory;
use crate::{AppError, CacheKey, DbKey, DictTable};
use lru::LruCache;
use redb::{Database, Key, MultimapTableDefinition, TableDefinition, WriteTransaction};
use std::fmt::Debug;
use std::num::NonZeroUsize;
use std::sync::Weak;

#[derive(Clone)]
pub struct DictFactory<K: Key + 'static, V: Key + 'static> {
    pub name: String,
    pub dict_pk_to_ids_def: MultimapTableDefinition<'static, K, K>,
    pub value_by_dict_pk_def: TableDefinition<'static, K, V>,
    pub value_to_dict_pk_def: TableDefinition<'static, V, K>,
    pub dict_pk_by_id_def: TableDefinition<'static, K, K>,
    pub lru_capacity: Option<usize>,
}

impl<K: Key + 'static, V: Key + 'static> Debug for DictFactory<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DictFactory").field("name", &self.name).finish()
    }
}

impl<K: Key + 'static, V: Key + 'static> DictFactory<K, V> {
    pub fn new(name: &str, lru_capacity: usize, dict_pk_to_ids_def: MultimapTableDefinition<'static, K, K>, value_by_dict_pk_def: TableDefinition<'static, K, V>, value_to_dict_pk_def: TableDefinition<'static, V, K>, dict_pk_by_id_def: TableDefinition<'static, K, K>) -> Self {
        let lru_cache_size_opt =
            if lru_capacity < 1 {
                None
            } else {
                Some(lru_capacity)
            };
        Self {
            name: name.to_string(),
            dict_pk_to_ids_def,
            value_by_dict_pk_def,
            value_to_dict_pk_def,
            dict_pk_by_id_def,
            lru_capacity: lru_cache_size_opt
        }
    }
}

impl<K: DbKey, V: CacheKey> TableFactory<K, V> for DictFactory<K, V> {
    type CacheCtx = Option<LruCache<V::CK, K::Unit>>;
    type Table<'txn, 'c> = DictTable<'txn, 'c, K, V>;
    type ReadOnlyTable = ReadOnlyDictTable<K, V>;

    fn name(&self) -> String {
        self.name.clone()
    }

    fn new_cache(&self) -> Self::CacheCtx {
        self.lru_capacity.map(|cap| LruCache::new(NonZeroUsize::new(cap).expect("lru_capacity for dictionary must be > 0")))
    }

    fn open_for_write<'txn, 'c>(
        &self,
        tx: &'txn WriteTransaction,
        cache: &'c mut Self::CacheCtx,
    ) -> redb::Result<Self::Table<'txn, 'c>, AppError> {
        DictTable::new(
            tx,
            cache.as_mut(),
            self.dict_pk_to_ids_def,
            self.value_by_dict_pk_def,
            self.value_to_dict_pk_def,
            self.dict_pk_by_id_def,
        )
    }

    fn open_for_read(&self, db_weak: &Weak<Database>) -> redb::Result<Self::ReadOnlyTable, AppError> {
        ReadOnlyDictTable::new(
            db_weak,
            self.dict_pk_to_ids_def,
            self.value_by_dict_pk_def,
            self.value_to_dict_pk_def,
            self.dict_pk_by_id_def,
        )
    }
}