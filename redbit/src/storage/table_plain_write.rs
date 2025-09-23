use crate::storage::table_writer::{TableFactory, ValueBuf, WriteTableLike};
use crate::AppError;
use redb::*;
use redb::{Key, Table, WriteTransaction};
use std::borrow::Borrow;
use std::ops::RangeBounds;

pub struct PlainFactory<K: Key + 'static, V: Key + 'static> {
    pub table_def: TableDefinition<'static, K, V>,
}

impl<K: Key + 'static, V: Key + 'static> PlainFactory<K, V> {
    pub fn new(table_def: TableDefinition<'static, K, V>) -> Self {
        Self {
            table_def,
        }
    }
}

pub struct PlainTable<'txn, K: Key + 'static, V: Key + 'static> {
    table: Table<'txn, K, V>,
}

impl<'txn, K: Key + 'static, V: Key + 'static> PlainTable<'txn, K, V> {
    pub fn new(write_tx: &'txn WriteTransaction, table_def: TableDefinition<'static, K, V>) -> Result<Self, AppError> {
        Ok(Self {
            table: write_tx.open_table(table_def)?,
        })
    }
}

impl<K: Key + 'static, V: Key + 'static> TableFactory<K, V> for PlainFactory<K, V> {
    type CacheCtx = ();
    type Table<'txn, 'c> = PlainTable<'txn, K, V>;

    fn new_cache(&self) -> Self::CacheCtx { }

    fn open<'txn, 'c>(&self, tx: &'txn WriteTransaction, _cache: &'c mut Self::CacheCtx) -> Result<Self::Table<'txn, 'c>, AppError> {
        PlainTable::new(tx, self.table_def)
    }
}

impl<'txn, K: Key + 'static, V: Key + 'static> WriteTableLike<K, V> for PlainTable<'txn, K, V> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<(), AppError>  {
        self.table.insert(key, value)?;
        Ok(())
    }

    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> Result<bool, AppError>  {
        let removed = self.table.remove(key)?;
        Ok(removed.is_some())
    }

    fn get_head_by_index<'v>(&mut self, _value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueBuf<K>>>  {
        unimplemented!()
    }

    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, range: impl RangeBounds<KR> + 'a) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>> {
        let mut result: Vec<(ValueBuf<K>, ValueBuf<V>)> = Vec::new();
        let mm = self.table.range(range);
        for tuple in mm? {
            let (k_guard, v_guard) = tuple?;
            result.push((Self::key_buf(k_guard), Self::value_buf(v_guard)));
        }
        Ok(result)
    }
}
