use crate::storage::table_writer::{TableFactory, ValueBuf, WriteTableLike};
use crate::AppError;
use redb::*;
use redb::{Key, Table, WriteTransaction};
use std::borrow::Borrow;
use std::num::NonZeroUsize;
use std::ops::RangeBounds;
use lru::LruCache;

#[derive(Clone)]
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
    pub(crate) pk_by_index: MultimapTable<'txn, V, K>,
    pub(crate) index_by_pk: Table<'txn, K, V>,
    pub(crate) cache: Option<&'c mut LruCache<Vec<u8>, Vec<u8>>>,
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
            if removed && let Some(c) = self.cache.as_mut() {
                let v_bytes = V::as_bytes(&value).as_ref().to_vec();
                let _ = c.pop(&v_bytes);
            }
            Ok(removed)
        } else {
            Ok(false)
        }
    }

    fn get_any_for_index<'v>(&mut self, value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueBuf<K>>, AppError> {
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

    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, _range: impl RangeBounds<KR> + 'a) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError> {
        unimplemented!()
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod index_write_table_tests {
    use super::*;
    use crate::storage::test_utils::{Address, addr};
    use crate::storage::index_test_utils::{setup_index_defs, mk_index};

    // Insert two PKs for the same Address; head should be the smallest PK.
    #[test]
    fn insert_and_head_returns_smallest_pk() {
        let (_owner_db, _, mut cache, pk_by_index_def, index_by_pk_def) = setup_index_defs(1_000);

        let tx   = _owner_db.begin_write().expect("begin write");
        let mut tbl: IndexTable<'_, '_, u32, Address> =
            mk_index(&tx, &mut cache, pk_by_index_def, index_by_pk_def);

        // Insert two primary keys for the same address
        let addr1 = addr(&[1, 2, 3, 4, 5]);
        tbl.insert_kv(&10u32, &addr1).expect("insert 10");
        tbl.insert_kv(&3u32,  &addr1).expect("insert 3");

        // Head is the smallest K (u32)
        let head = tbl.get_any_for_index(&addr1).expect("get_head").expect("expected Some");
        let k: u32 = {
            // ValueBuf<K> -> K via K::from_bytes
            let got = <u32 as Value>::from_bytes(head.as_bytes());
            got
        };
        assert_eq!(k, 3, "head must be the smallest PK for a given index value");
    }

    // Deleting one PK keeps the other; deleting the last removes the mapping.
    #[test]
    fn delete_kv_updates_both_directions_and_clears_last() {
        let (_owner_db, _, mut cache, pk_by_index_def, index_by_pk_def) = setup_index_defs(1_000);

        let tx   = _owner_db.begin_write().expect("begin write");
        let mut tbl: IndexTable<'_, '_, u32, Address> =
            mk_index(&tx, &mut cache, pk_by_index_def, index_by_pk_def);

        let addr1 = addr(&[9, 9, 9]);
        tbl.insert_kv(&10u32, &addr1).expect("insert 10");
        tbl.insert_kv(&7u32,  &addr1).expect("insert 7");
        tbl.insert_kv(&42u32, &addr(&[8,8,8])).expect("insert 42 other");

        // Head is the smallest among {10,7} => 7
        let head_before = tbl.get_any_for_index(&addr1).unwrap().unwrap();
        let k_before: u32 = <u32 as Value>::from_bytes(head_before.as_bytes());
        assert_eq!(k_before, 7);

        // Delete the head (7) — 10 should remain as the new head
        assert!(tbl.delete_kv(&7u32).expect("delete 7"));
        let head_after = tbl.get_any_for_index(&addr1).unwrap().unwrap();
        let k_after: u32 = <u32 as Value>::from_bytes(head_after.as_bytes());
        assert_eq!(k_after, 10);

        // Delete the last (10) — mapping should disappear
        assert!(tbl.delete_kv(&10u32).expect("delete 10"));
        let none_now = tbl.get_any_for_index(&addr1).unwrap();
        assert!(none_now.is_none(), "no PKs should remain for addr1");

        // Deleting non-existent key returns false
        assert!(!tbl.delete_kv(&999u32).expect("delete absent"));
    }

    // Cache fast path: pre-populate LRU (value->key bytes) and ensure get_any_for_index hits it
    // even when tables have no entries for that value.
    #[test]
    fn cache_fast_path_serves_head_without_table_lookup() {
        // Build and pre-populate cache BEFORE constructing table (it borrows &mut cache).
        let (_owner_db, _, mut cache, pk_by_index_def, index_by_pk_def) = setup_index_defs(128);

        let tx   = _owner_db.begin_write().expect("begin write");
        // Prepare a value (address) and its key (u32) in LE bytes
        let value_addr = addr(&[0xAA, 0xBB, 0xCC, 0xDD]);
        let k: u32 = 12345;
        let v_bytes = <Address as Value>::as_bytes(&value_addr).as_ref().to_vec();
        let k_bytes = <u32 as Value>::as_bytes(&k).as_ref().to_vec();

        // Manually seed cache (value -> key) so get_any_for_index hits the fast path
        {
            assert!(cache.cap().get() >= 1);
            cache.put(v_bytes.clone(), k_bytes.clone());
        }

        let mut tbl: IndexTable<'_, '_, u32, Address> =
            mk_index(&tx, &mut cache, pk_by_index_def, index_by_pk_def);

        // No table inserts; get_any_for_index should return from cache
        let any = tbl.get_any_for_index(&value_addr).expect("cache path").expect("Some");
        let got_k: u32 = <u32 as Value>::from_bytes(any.as_bytes());
        assert_eq!(got_k, k, "cache fast path must return the cached head key");

        // Sanity: the tables still don't have this mapping; deleting should return false.
        assert!(!tbl.delete_kv(&k).expect("delete_kv on non-existent table row should be false"));
    }
}
