use crate::storage::table_writer::{TableFactory, WriteTableLike};
use crate::{AppError, CacheKey};
use redb::*;
use redb::{Key, Table, WriteTransaction};
use std::borrow::Borrow;
use std::num::NonZeroUsize;
use std::ops::RangeBounds;
use lru::LruCache;
use crate::storage::async_boundary::{CopyOwnedValue, ValueBuf, ValueOwned};

#[derive(Clone)]
pub struct IndexFactory<K: Key + 'static, V: Key + 'static> {
    pub(crate) name: String,
    pub(crate) pk_by_index_def: MultimapTableDefinition<'static, V, K>,
    pub(crate) index_by_pk_def: TableDefinition<'static, K, V>,
    pub(crate) lru_capacity: Option<usize>,
}

impl<K: Key + 'static, V: Key + 'static> IndexFactory<K, V> {
    pub fn new(name: &str, lru_capacity: usize, pk_by_index_def: MultimapTableDefinition<'static, V, K>, index_by_pk_def: TableDefinition<'static, K, V>) -> Self {
        let lru_cache_size_opt =
            if lru_capacity < 1 {
                None
            } else {
                Some(lru_capacity)
            };
        Self {
            name: name.to_string(),
            pk_by_index_def,
            index_by_pk_def,
            lru_capacity: lru_cache_size_opt
        }
    }
}

pub struct IndexTable<'txn, 'c, K: CopyOwnedValue + 'static, V: CacheKey + 'static> {
    pub(crate) pk_by_index: MultimapTable<'txn, V, K>,
    pub(crate) index_by_pk: Table<'txn, K, V>,
    pub(crate) cache: Option<&'c mut LruCache<V::CK, K::Unit>>,
}

impl<'txn, 'c, K: Key + CopyOwnedValue + 'static, V: CacheKey + 'static> IndexTable<'txn, 'c, K, V> {
    pub fn new(write_tx: &'txn WriteTransaction, cache: Option<&'c mut LruCache<V::CK, K::Unit>>, pk_by_index_def: MultimapTableDefinition<'static, V, K>, index_by_pk_def: TableDefinition<'static, K, V>) -> Result<Self, AppError> {
        Ok(Self {
            pk_by_index: write_tx.open_multimap_table(pk_by_index_def)?,
            index_by_pk: write_tx.open_table(index_by_pk_def)?,
            cache
        })
    }
}

impl<K: Key + CopyOwnedValue + 'static, V: CacheKey + 'static> TableFactory<K, V> for IndexFactory<K, V> {
    type CacheCtx = Option<LruCache<V::CK, K::Unit>>;
    type Table<'txn, 'c> = IndexTable<'txn, 'c, K, V>;

    fn name(&self) -> String {
        self.name.clone()
    }

    fn new_cache(&self) -> Self::CacheCtx {
        self.lru_capacity.map(|cap| LruCache::new(NonZeroUsize::new(cap).expect("lru_capacity for index must be > 0")))
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

impl<'txn, 'c, K: Key + CopyOwnedValue + 'static, V: CacheKey + 'static> WriteTableLike<K, V> for IndexTable<'txn, 'c, K, V> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<(), AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        let val_ref: &V::SelfType<'v> = value.borrow();
        self.index_by_pk.insert(key_ref, val_ref)?;
        self.pk_by_index.insert(val_ref, key_ref)?;

        if let Some(c) = self.cache.as_mut() {
            c.put(V::cache_key(val_ref), Self::unit_from_key(key_ref));
        }
        Ok(())
    }

    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> Result<bool, AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        if let Some(value_guard) = self.index_by_pk.remove(key_ref)? {
            let value = value_guard.value();
            let removed = self.pk_by_index.remove(&value, key_ref)?;
            if removed && let Some(c) = self.cache.as_mut() {
                let _ = c.pop(&V::cache_key(&value));
            }
            Ok(removed)
        } else {
            Ok(false)
        }
    }

    fn get_any_for_index<'v>(&mut self, value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueOwned<K>>, AppError> {
        if let Some(c) = self.cache.as_mut() {
            if let Some(&k) = c.get(&V::cache_key(value.borrow())) {
                return Ok(Some(Self::owned_from_unit(k)));
            }
        }

        let mut it = self.pk_by_index.get(value)?;
        if let Some(g) = it.next() {
            Ok(Some(Self::owned_key_from_guard(g?)))
        } else {
            Ok(None)
        }
    }
    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, _range: impl RangeBounds<KR> + 'a) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError> {
        unimplemented!()
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use super::*;
    use crate::storage::test_utils::TxHash;
    use crate::storage::index_test_utils::{setup_index_defs, mk_index};
    use crate::storage::test_utils;

    // Insert two PKs for the same TxHash; head should be the smallest PK.
    #[test]
    fn insert_and_head_returns_smallest_pk() {
        // setup_index_defs must be generic over V=TxHash (per your earlier changes)
        let name = "insert_and_head_returns_smallest_pk";
        let (_owner_db, _, mut cache, pk_by_index_def, index_by_pk_def) = setup_index_defs::<u32, TxHash>(name, 1_000);

        let tx = _owner_db.begin_write().expect("begin write");
        let mut tbl: IndexTable<'_, '_, u32, TxHash> =
            mk_index(&tx, &mut cache, pk_by_index_def, index_by_pk_def);

        // Insert two primary keys for the same TxHash
        let h1 = test_utils::txh(&[1, 2, 3, 4, 5]);
        tbl.insert_kv(&10u32, &h1).expect("insert 10");
        tbl.insert_kv(&3u32,  &h1).expect("insert 3");

        // Head is the smallest K (u32)
        let head_owned = tbl.get_any_for_index(&h1)
            .expect("get_any")
            .expect("expected Some")
            .as_value(); // ValueOwned<u32> -> u32
        assert_eq!(head_owned, 3, "head must be the smallest PK for a given index value");
    }

    // Deleting one PK keeps the other; deleting the last removes the mapping.
    #[test]
    fn delete_kv_updates_both_directions_and_clears_last() {
        let name = "insert_and_head_returns_smallest_pk";
        let (_owner_db, _, mut cache, pk_by_index_def, index_by_pk_def) = setup_index_defs::<u32, TxHash>(name, 1_000);

        let tx = _owner_db.begin_write().expect("begin write");
        let mut tbl: IndexTable<'_, '_, u32, TxHash> =
            mk_index(&tx, &mut cache, pk_by_index_def, index_by_pk_def);

        let h = test_utils::txh(&[9, 9, 9]);
        tbl.insert_kv(&10u32, &h).expect("insert 10");
        tbl.insert_kv(&7u32,  &h).expect("insert 7");
        tbl.insert_kv(&42u32, &test_utils::txh(&[8, 8, 8])).expect("insert 42 other");

        // Head is the smallest among {10,7} => 7
        let head_before = tbl.get_any_for_index(&h).unwrap().unwrap().as_value();
        assert_eq!(head_before, 7);

        // Delete the head (7) — 10 should remain as the new head
        assert!(tbl.delete_kv(&7u32).expect("delete 7"));
        let head_after = tbl.get_any_for_index(&h).unwrap().unwrap().as_value();
        assert_eq!(head_after, 10);

        // Delete the last (10) — mapping should disappear
        assert!(tbl.delete_kv(&10u32).expect("delete 10"));
        let none_now = tbl.get_any_for_index(&h).unwrap();
        assert!(none_now.is_none(), "no PKs should remain for this hash");

        // Deleting non-existent key returns false
        assert!(!tbl.delete_kv(&999u32).expect("delete absent"));
    }

    // Cache fast path: pre-populate LRU (value->key Unit) and ensure get_any_for_index hits it
    // even when tables have no entries for that value.
    #[test]
    fn cache_fast_path_serves_any_without_table_lookup() {
        let name = "cache_fast_path_serves_any_without_table_lookup";
        let (_owner_db, _, mut cache, pk_by_index_def, index_by_pk_def) = setup_index_defs::<u32, TxHash>(name, 128);

        let tx = _owner_db.begin_write().expect("begin write");

        // Prepare a TxHash (value) and its key (u32) in the cache's types:
        let value_hash = test_utils::txh(&[0xAA, 0xBB, 0xCC, 0xDD]);
        let k: u32 = 12345;

        // Cache key (typed) + unit value for K
        let ckey = <TxHash as CacheKey>::cache_key(&value_hash);
        let kunit = <u32 as CopyOwnedValue>::to_unit(k);

        // Seed cache so get_any_for_index hits fast path
        assert!(cache.cap().get() >= 1);
        cache.put(ckey, kunit);

        let mut tbl: IndexTable<'_, '_, u32, TxHash> = mk_index(&tx, &mut cache, pk_by_index_def, index_by_pk_def);

        // No table inserts; get_any_for_index should return from cache
        let any = tbl.get_any_for_index(&value_hash)
            .expect("cache path")
            .expect("Some")
            .as_value();
        assert_eq!(any, k, "cache fast path must return the cached head key");

        // Sanity: the tables still don't have this mapping; deleting should return false.
        assert!(!tbl.delete_kv(&k).expect("delete_kv on non-existent table row should be false"));
    }
}
