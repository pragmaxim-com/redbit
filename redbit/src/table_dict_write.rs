use crate::table_writer::{TableFactory, ValueBuf, WriteTableLike};
use crate::AppError;
use redb::*;
use redb::{Key, Table, WriteTransaction};
use std::borrow::Borrow;
use std::ops::RangeBounds;

pub struct DictFactory<K: Key + 'static, V: Key + 'static> {
    pub dict_index_def: MultimapTableDefinition<'static, K, K>,
    pub by_dict_pk_def: TableDefinition<'static, K, V>,
    pub to_dict_pk_def: TableDefinition<'static, V, K>,
    pub dict_pk_by_id_def: TableDefinition<'static, K, K>,
}

impl<K: Key + 'static, V: Key + 'static> DictFactory<K, V> {
    pub fn new(dict_index_def: MultimapTableDefinition<'static, K, K>, by_dict_pk_def: TableDefinition<'static, K, V>, to_dict_pk_def: TableDefinition<'static, V, K>, dict_pk_by_id_def: TableDefinition<'static, K, K>) -> Self {
        Self {
            dict_index_def,
            by_dict_pk_def,
            to_dict_pk_def,
            dict_pk_by_id_def,
        }
    }
}

impl<K: Key + 'static, V: Key + 'static> TableFactory<K, V> for DictFactory<K, V> {
    type Table<'txn> = DictTable<'txn, K, V>;

    fn open<'txn>(&self, tx: &'txn WriteTransaction) -> Result<Self::Table<'txn>, AppError> {
        DictTable::new(
            tx,
            self.dict_index_def,
            self.by_dict_pk_def,
            self.to_dict_pk_def,
            self.dict_pk_by_id_def,
        )
    }
}

pub struct DictTable<'txn, K: Key + 'static, V: Key + 'static> {
    dict_index: MultimapTable<'txn, K, K>,
    by_dict_pk: Table<'txn, K, V>,
    to_dict_pk: Table<'txn, V, K>,
    dict_pk_by_id: Table<'txn, K, K>,
}

impl<'txn, K: Key + 'static, V: Key + 'static> DictTable<'txn, K, V> {
    pub fn new(write_tx: &'txn WriteTransaction, dict_index_def: MultimapTableDefinition<K, K>, by_dict_pk_def: TableDefinition<K, V>, to_dict_pk_def: TableDefinition<V, K>, dict_pk_by_id_def: TableDefinition<K, K>) -> Result<Self, AppError> {
        Ok(Self {
            dict_index: write_tx.open_multimap_table(dict_index_def)?,
            by_dict_pk: write_tx.open_table(by_dict_pk_def)?,
            to_dict_pk: write_tx.open_table(to_dict_pk_def)?,
            dict_pk_by_id: write_tx.open_table(dict_pk_by_id_def)?,
        })
    }
}

impl<'txn, K: Key + 'static, V: Key + 'static> WriteTableLike<'txn, K, V> for DictTable<'txn, K, V> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<(), AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        let val_ref: &V::SelfType<'v> = value.borrow();

        if let Some(birth_id_guard) = self.to_dict_pk.get(val_ref)? {
            let birth_id = birth_id_guard.value();
            self.dict_pk_by_id.insert(key_ref, &birth_id)?;
            self.dict_index.insert(birth_id, key_ref)?;
        } else {
            self.to_dict_pk.insert(val_ref, key_ref)?;
            self.by_dict_pk.insert(key_ref, val_ref)?;
            self.dict_pk_by_id.insert(key_ref, key_ref)?;
            self.dict_index.insert(key_ref, key_ref)?;
        }
        Ok(())
    }

    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> Result<bool, AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        if let Some(birth_guard) = self.dict_pk_by_id.remove(key_ref)? {
            let birth_id = birth_guard.value();
            let was_removed = self.dict_index.remove(&birth_id, key_ref)?;
            if self.dict_index.get(&birth_id)?.is_empty() {
                if let Some(value_guard) = self.by_dict_pk.remove(&birth_id)? {
                    let value = value_guard.value();
                    self.to_dict_pk.remove(&value)?;
                }
            }
            Ok(was_removed)
        } else {
            Ok(false)
        }
    }

    fn get_head_by_index<'v>(&self, _value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueBuf<K>>>  {
        unimplemented!()
    }

    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, _range: impl RangeBounds<KR> + 'a) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>> {
        unimplemented!()
    }
}
