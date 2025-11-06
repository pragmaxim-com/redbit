use std::fmt::Debug;
use std::sync::Weak;
use redb::{Database, Key, Table, TableDefinition, WriteTransaction};
use crate::{AppError, CopyOwnedValue};
use crate::storage::table_plain_read::ReadOnlyPlainTable;
use crate::storage::table_writer_api::TableFactory;

#[derive(Clone)]
pub struct PlainFactory<K: Key + 'static, V: Key + 'static> {
    pub(crate) name: String,
    pub(crate) table_def: TableDefinition<'static, K, V>,
}

impl<K: Key + 'static, V: Key + 'static> Debug for PlainFactory<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlainFactory").field("name", &self.name).finish()
    }
}

impl<K: Key + 'static, V: Key + 'static> PlainFactory<K, V> {
    pub fn new(name: &str, table_def: TableDefinition<'static, K, V>) -> Self {
        Self {
            name: name.to_string(),
            table_def,
        }
    }
}

pub struct PlainTable<'txn, K: Key + 'static, V: Key + 'static> {
    pub(crate) table: Table<'txn, K, V>,
}

impl<'txn, K: Key + 'static, V: Key + 'static> PlainTable<'txn, K, V> {
    pub fn new(write_tx: &'txn WriteTransaction, table_def: TableDefinition<'static, K, V>) -> redb::Result<Self, AppError> {
        Ok(Self {
            table: write_tx.open_table(table_def)?,
        })
    }
}

impl<K: CopyOwnedValue + 'static, V: Key + 'static> TableFactory<K, V> for PlainFactory<K, V> {
    type CacheCtx = ();
    type Table<'txn, 'c> = PlainTable<'txn, K, V>;
    type ReadOnlyTable = ReadOnlyPlainTable<K, V>;

    fn name(&self) -> String {
        self.name.clone()
    }

    fn new_cache(&self) -> Self::CacheCtx { }

    fn open_for_write<'txn, 'c>(&self, tx: &'txn WriteTransaction, _cache: &'c mut Self::CacheCtx) -> redb::Result<Self::Table<'txn, 'c>, AppError> {
        PlainTable::new(tx, self.table_def)
    }

    fn open_for_read(&self, db_weak: &Weak<Database>) -> redb::Result<Self::ReadOnlyTable, AppError> {
        ReadOnlyPlainTable::new(db_weak, self.table_def)
    }
}
