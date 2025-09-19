use crate::storage::table_writer::{TableFactory, ValueBuf, WriteTableLike};
use crate::AppError;
use redb::*;
use redb::{Key, Table, WriteTransaction};
use std::borrow::Borrow;
use std::num::NonZeroUsize;
use std::ops::RangeBounds;
use lru::LruCache;

pub struct IndexFactory<K: Key + 'static, V: Key + 'static> {
    pub pk_by_index_def: MultimapTableDefinition<'static, V, K>,
    pub index_by_pk_def: TableDefinition<'static, K, V>,
    pub lru_capacity: Option<usize>,
}

impl<K: Key + 'static, V: Key + 'static> IndexFactory<K, V> {
    pub fn new(lru_capacity: usize, pk_by_index_def: MultimapTableDefinition<'static, V, K>, index_by_pk_def: TableDefinition<'static, K, V>) -> Self {
        let lru_cache_size_opt =
            if lru_capacity < 1 {
                None
            } else {
                Some(lru_capacity)
            };
        Self {
            pk_by_index_def,
            index_by_pk_def,
            lru_capacity: lru_cache_size_opt
        }
    }
}

pub struct IndexTable<'txn, 'c, K: Key + 'static, V: Key + 'static> {
    pk_by_index: MultimapTable<'txn, V, K>,
    index_by_pk: Table<'txn, K, V>,
    cache: Option<&'c mut LruCache<Vec<u8>, Vec<u8>>>,
}

impl<'txn, 'c, K: Key + 'static, V: Key + 'static> IndexTable<'txn, 'c, K, V> {
    pub fn new(write_tx: &'txn WriteTransaction, cache: Option<&'c mut LruCache<Vec<u8>, Vec<u8>>>, pk_by_index_def: MultimapTableDefinition<'static, V, K>, index_by_pk_def: TableDefinition<'static, K, V>) -> Result<Self, AppError> {
        Ok(Self {
            pk_by_index: write_tx.open_multimap_table(pk_by_index_def)?,
            index_by_pk: write_tx.open_table(index_by_pk_def)?,
            cache
        })
    }
}

impl<K: Key + 'static, V: Key + 'static> TableFactory<K, V> for IndexFactory<K, V> {
    type CacheCtx = Option<LruCache<Vec<u8>, Vec<u8>>>;
    type Table<'txn, 'c> = IndexTable<'txn, 'c, K, V>;

    fn new_cache(&self) -> Self::CacheCtx {
        self.lru_capacity.map(|cap| LruCache::new(NonZeroUsize::new(cap).unwrap()))
    }

    fn open<'txn, 'c>(
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
}

impl<'txn, 'c, K: Key + 'static, V: Key + 'static> WriteTableLike<K, V> for IndexTable<'txn, 'c, K, V> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<(), AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        let val_ref: &V::SelfType<'v> = value.borrow();
        self.index_by_pk.insert(key_ref, val_ref)?;
        self.pk_by_index.insert(val_ref, key_ref)?;

        if let Some(c) = self.cache.as_mut() {
            let v_bytes = V::as_bytes(val_ref).as_ref().to_vec();
            let k_bytes = K::as_bytes(key_ref).as_ref().to_vec();
            c.put(v_bytes, k_bytes);
        }
        Ok(())
    }

    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> Result<bool, AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        if let Some(value_guard) = self.index_by_pk.remove(key_ref)? {
            let value = value_guard.value();
            let removed = self.pk_by_index.remove(&value, key_ref)?;
            if removed {
                if let Some(c) = self.cache.as_mut() {
                    let v_bytes = V::as_bytes(&value).as_ref().to_vec();
                    let _ = c.pop(&v_bytes);
                }
            }
            Ok(removed)
        } else {
            Ok(false)
        }
    }

    fn get_head_by_index<'v>(&mut self, value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueBuf<K>>> {
        // 1) cache fast path
        if let Some(c) = self.cache.as_mut() {
            let v_bytes_key = V::as_bytes(value.borrow());
            if let Some(k_bytes) = c.get(v_bytes_key.as_ref()) {
                return Ok(Some(ValueBuf::<K>::new(k_bytes.clone())));
            }
        }
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
