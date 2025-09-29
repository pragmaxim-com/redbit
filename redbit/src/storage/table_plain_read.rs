use crate::{AppError, TableInfo};
use redb::{AccessGuard, Database, Key, ReadOnlyTable, ReadableDatabase, ReadableTableMetadata, TableDefinition};
use std::borrow::Borrow;
use std::sync::Weak;

pub struct ReadOnlyPlainTable<K: Key + 'static, V: Key + 'static> {
    pub underlying: ReadOnlyTable<K, V>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyPlainTable<K, V> {
    pub fn new(index_db: Weak<Database>, underlying_def: TableDefinition<K, V>) -> redb::Result<Self, AppError> {
        let db_arc = index_db.upgrade().ok_or_else(|| AppError::Custom("database closed".to_string()))?;
        let index_tx = db_arc.begin_read()?;
        Ok(Self {
            underlying: index_tx.open_table(underlying_def)?,
        })
    }

    pub fn get_value<'k>(&self, key: impl Borrow<K::SelfType<'k>>, ) -> redb::Result<Option<AccessGuard<'_, V>>> {
        self.underlying.get(key.borrow())
    }

    pub fn stats(&self) -> redb::Result<TableInfo> {
        Ok(TableInfo::from_stats("underlying", self.underlying.len()?, self.underlying.stats()?))
    }
}