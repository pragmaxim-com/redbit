use crate::storage::table_writer::{TableFactory, ValueBuf, WriteTableLike};
use crate::AppError;
use redb::*;
use redb::{Key, Table, WriteTransaction};
use std::borrow::Borrow;
use std::num::NonZeroUsize;
use std::ops::RangeBounds;
use lru::LruCache;

pub struct DictFactory<K: Key + 'static, V: Key + 'static> {
    pub dict_index_def: MultimapTableDefinition<'static, K, K>,
    pub by_dict_pk_def: TableDefinition<'static, K, V>,
    pub to_dict_pk_def: TableDefinition<'static, V, K>,
    pub dict_pk_by_id_def: TableDefinition<'static, K, K>,
    pub lru_capacity: usize,
}

impl<K: Key + 'static, V: Key + 'static> DictFactory<K, V> {
    pub fn new( lru_capacity: usize, dict_index_def: MultimapTableDefinition<'static, K, K>, by_dict_pk_def: TableDefinition<'static, K, V>, to_dict_pk_def: TableDefinition<'static, V, K>, dict_pk_by_id_def: TableDefinition<'static, K, K>) -> Self {
        Self {
            dict_index_def,
            by_dict_pk_def,
            to_dict_pk_def,
            dict_pk_by_id_def,
            lru_capacity
        }
    }
}

impl<K: Key + 'static, V: Key + 'static> TableFactory<K, V> for DictFactory<K, V> {
    type CacheCtx = LruCache<Vec<u8>, Vec<u8>>;
    type Table<'txn, 'c> = DictTable<'txn, 'c, K, V>;

    fn new_cache(&self) -> Self::CacheCtx {
        LruCache::new(NonZeroUsize::new(self.lru_capacity).unwrap())
    }

    fn open<'txn, 'c>(
        &self,
        tx: &'txn WriteTransaction,
        cache: &'c mut Self::CacheCtx,
    ) -> Result<Self::Table<'txn, 'c>, AppError> {
        DictTable::new(
            tx,
            cache,
            self.dict_index_def,
            self.by_dict_pk_def,
            self.to_dict_pk_def,
            self.dict_pk_by_id_def,
        )
    }
}
pub struct DictTable<'txn, 'c, K: Key + 'static, V: Key + 'static> {
    dict_index: MultimapTable<'txn, K, K>,
    value_by_dict_pk: Table<'txn, K, V>,
    value_to_dict_pk: Table<'txn, V, K>,
    dict_pk_by_id: Table<'txn, K, K>,
    cache: &'c mut LruCache<Vec<u8>, Vec<u8>>,
}

impl<'txn, 'c, K: Key + 'static, V: Key + 'static> DictTable<'txn, 'c, K, V> {
    pub fn new(
        write_tx: &'txn WriteTransaction,
        cache: &'c mut LruCache<Vec<u8>, Vec<u8>>,
        dict_index_def: MultimapTableDefinition<K, K>,
        by_dict_pk_def: TableDefinition<K, V>,
        to_dict_pk_def: TableDefinition<V, K>,
        dict_pk_by_id_def: TableDefinition<K, K>,
    ) -> Result<Self, AppError> {
        Ok(Self {
            dict_index: write_tx.open_multimap_table(dict_index_def)?,
            value_by_dict_pk: write_tx.open_table(by_dict_pk_def)?,
            value_to_dict_pk: write_tx.open_table(to_dict_pk_def)?,
            dict_pk_by_id: write_tx.open_table(dict_pk_by_id_def)?,
            cache,
        })
    }
}
impl<'txn, 'c, K: Key + 'static, V: Key + 'static> WriteTableLike<'txn, K, V> for DictTable<'txn, 'c, K, V> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<(), AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        let val_ref: &V::SelfType<'v> = value.borrow();

        let v_bytes_key = V::as_bytes(val_ref);
        if let Some(k_bytes) = self.cache.get(v_bytes_key.as_ref()) {
            let birth_id = K::from_bytes(k_bytes);
            self.dict_pk_by_id.insert(key_ref, &birth_id)?;
            self.dict_index.insert(birth_id, key_ref)?;
            Ok(())
        } else {
            if let Some(birth_id_guard) = self.value_to_dict_pk.get(val_ref)? {
                let birth_id = birth_id_guard.value();
                let v_bytes = v_bytes_key.as_ref().to_vec();
                let k_bytes = K::as_bytes(&birth_id).as_ref().to_vec();
                self.cache.put(v_bytes, k_bytes);

                self.dict_pk_by_id.insert(key_ref, &birth_id)?;
                self.dict_index.insert(birth_id, key_ref)?;
            } else {
                self.value_to_dict_pk.insert(val_ref, key_ref)?;
                self.value_by_dict_pk.insert(key_ref, val_ref)?;
                self.dict_pk_by_id.insert(key_ref, key_ref)?;
                self.dict_index.insert(key_ref, key_ref)?;

                let v_bytes = v_bytes_key.as_ref().to_vec();
                let k_bytes = K::as_bytes(key_ref).as_ref().to_vec();
                self.cache.put(v_bytes, k_bytes);
            }
            Ok(())
        }
    }

    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> Result<bool, AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        if let Some(birth_guard) = self.dict_pk_by_id.remove(key_ref)? {
            let birth_id = birth_guard.value();
            let was_removed = self.dict_index.remove(&birth_id, key_ref)?;
            if self.dict_index.get(&birth_id)?.is_empty() {
                if let Some(value_guard) = self.value_by_dict_pk.remove(&birth_id)? {
                    let value = value_guard.value();
                    self.value_to_dict_pk.remove(&value)?;

                    // evict from cache (value -> dict_pk)
                    let v_bytes = V::as_bytes(&value).as_ref().to_vec();
                    let _ = self.cache.pop(&v_bytes);
                }
            }
            Ok(was_removed)
        } else {
            Ok(false)
        }
    }

    fn get_head_by_index<'v>(&mut self, _value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueBuf<K>>>  {
        unimplemented!()
    }

    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, _range: impl RangeBounds<KR> + 'a) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>> {
        unimplemented!()
    }
}
