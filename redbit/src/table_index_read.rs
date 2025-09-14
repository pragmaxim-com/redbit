use std::borrow::Borrow;
use std::collections::HashMap;
use std::ops::RangeBounds;
use std::sync::Arc;
use redb::{AccessGuard, Database, Key, MultimapTableDefinition, MultimapValue, ReadOnlyMultimapTable, ReadOnlyTable, ReadableDatabase, ReadableTableMetadata, TableDefinition, TableStats};
use crate::AppError;

pub struct ReadOnlyIndexTable<K: Key + 'static, V: Key + 'static> {
    pk_by_index: ReadOnlyMultimapTable<V, K>,
    index_by_pk: ReadOnlyTable<K, V>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyIndexTable<K, V> {
    pub fn new(index_db: Arc<Database>, pk_by_index_def: MultimapTableDefinition<V, K>, index_by_pk_def: TableDefinition<K, V>) -> redb::Result<Self, AppError> {
        let index_tx = index_db.begin_read()?;
        Ok(Self {
            pk_by_index: index_tx.open_multimap_table(pk_by_index_def)?,
            index_by_pk: index_tx.open_table(index_by_pk_def)?,
        })
    }

    pub fn get_value<'k>(&self, key: impl Borrow<K::SelfType<'k>>,) -> redb::Result<Option<AccessGuard<'_, V>>> {
        self.index_by_pk.get(key.borrow())
    }

    pub fn get_keys<'v>(&self, val: impl Borrow<V::SelfType<'v>>) -> redb::Result<MultimapValue<'static, K>> {
        self.pk_by_index.get(val.borrow())

    }

    pub fn range_keys<'a, KR: Borrow<V::SelfType<'a>>>(&self, range: impl RangeBounds<KR>) -> redb::Result<redb::MultimapRange<'static, V, K>> {
        self.pk_by_index.range(range)
    }

    pub fn stats(&self) -> redb::Result<HashMap<String, TableStats>> {
        let mut stats = HashMap::new();
        stats.insert("pk_by_index".to_string(), self.pk_by_index.stats()?);
        stats.insert("index_by_pk".to_string(), self.index_by_pk.stats()?);
        Ok(stats)
    }
}
