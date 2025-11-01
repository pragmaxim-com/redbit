use crate::{AppError, TableInfo};
use redb::{AccessGuard, Database, Key, MultimapTableDefinition, MultimapValue, ReadOnlyMultimapTable, ReadOnlyTable, ReadableDatabase, ReadableTableMetadata, TableDefinition};
use std::borrow::Borrow;
use std::ops::RangeBounds;
use std::sync::Weak;

pub struct ReadOnlyIndexTable<K: Key + 'static, V: Key + 'static> {
    pk_by_index: ReadOnlyMultimapTable<V, K>,
    index_by_pk: ReadOnlyTable<K, V>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyIndexTable<K, V> {
    pub fn new(index_db: Weak<Database>, pk_by_index_def: MultimapTableDefinition<V, K>, index_by_pk_def: TableDefinition<K, V>) -> redb::Result<Self, AppError> {
        let db_arc = index_db.upgrade().ok_or_else(|| AppError::Custom("database closed".to_string()))?;
        let index_tx = db_arc.begin_read()?;
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

    pub fn stats(&self) -> redb::Result<Vec<TableInfo>> {
        Ok(
            vec![
                TableInfo::from_stats("pk_by_index", self.pk_by_index.len()?, self.pk_by_index.stats()?),
                TableInfo::from_stats("index_by_pk", self.index_by_pk.len()?, self.index_by_pk.stats()?),
            ]
        )
    }
}
