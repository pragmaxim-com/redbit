use crate::table_writer::{TableFactory, ValueBuf, WriteTableLike};
use crate::AppError;
use redb::*;
use redb::{Key, Table, WriteTransaction};
use std::borrow::Borrow;
use std::ops::RangeBounds;

pub struct IndexFactory<K: Key + 'static, V: Key + 'static> {
    pub pk_by_index_def: MultimapTableDefinition<'static, V, K>,
    pub index_by_pk_def: TableDefinition<'static, K, V>,
}

impl<K: Key + 'static, V: Key + 'static> IndexFactory<K, V> {
    pub fn new(pk_by_index_def: MultimapTableDefinition<'static, V, K>, index_by_pk_def: TableDefinition<'static, K, V>) -> Self {
        Self {
            pk_by_index_def,
            index_by_pk_def,
        }
    }
}

pub struct IndexTable<'txn, K: Key + 'static, V: Key + 'static> {
    pk_by_index: MultimapTable<'txn, V, K>,
    index_by_pk: Table<'txn, K, V>,
}

impl<'txn, K: Key + 'static, V: Key + 'static> IndexTable<'txn, K, V> {
    pub fn new(write_tx: &'txn WriteTransaction, pk_by_index_def: MultimapTableDefinition<'static, V, K>, index_by_pk_def: TableDefinition<'static, K, V>) -> Result<Self, AppError> {
        Ok(Self {
            pk_by_index: write_tx.open_multimap_table(pk_by_index_def)?,
            index_by_pk: write_tx.open_table(index_by_pk_def)?,
        })
    }
}

impl<K: Key + 'static, V: Key + 'static> TableFactory<K, V> for IndexFactory<K, V> {
    type Table<'txn> = IndexTable<'txn, K, V>;

    fn open<'txn>(&self, tx: &'txn WriteTransaction) -> Result<Self::Table<'txn>, AppError> {
        IndexTable::new(tx, self.pk_by_index_def, self.index_by_pk_def)
    }
}


impl<'txn, K: Key + 'static, V: Key + 'static> WriteTableLike<'txn, K, V> for IndexTable<'txn, K, V> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<(), AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        let val_ref: &V::SelfType<'v> = value.borrow();
        self.index_by_pk.insert(key_ref, val_ref)?;
        self.pk_by_index.insert(val_ref, key_ref)?;
        Ok(())
    }

    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> Result<bool, AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        if let Some(value_guard) = self.index_by_pk.remove(key_ref)? {
            let value = value_guard.value();
            let removed = self.pk_by_index.remove(value, key_ref)?;
            Ok(removed)
        } else {
            Ok(false)
        }
    }

    fn get_head_by_index<'v>(&self, value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueBuf<K>>> {
        let mut result = self.pk_by_index.get(value)?;
        if let Some(guard) = result.next() {
            Ok(Some(Self::key_buf(guard?)))
        } else {
            Ok(None)
        }
    }

    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, _range: impl RangeBounds<KR> + 'a) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>> {
        unimplemented!()
    }
}
