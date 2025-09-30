use crate::storage::table_writer::{TableFactory, ValueBuf, WriteTableLike};
use crate::AppError;
use redb::*;
use redb::{Key, Table, WriteTransaction};
use std::borrow::Borrow;
use std::num::NonZeroUsize;
use std::ops::RangeBounds;
use lru::LruCache;

#[derive(Clone)]
pub struct DictFactory<K: Key + 'static, V: Key + 'static> {
    pub dict_pk_to_ids_def: MultimapTableDefinition<'static, K, K>,
    pub value_by_dict_pk_def: TableDefinition<'static, K, V>,
    pub value_to_dict_pk_def: TableDefinition<'static, V, K>,
    pub dict_pk_by_id_def: TableDefinition<'static, K, K>,
    pub lru_capacity: usize,
}

impl<K: Key + 'static, V: Key + 'static> DictFactory<K, V> {
    pub fn new( lru_capacity: usize, dict_pk_to_ids_def: MultimapTableDefinition<'static, K, K>, value_by_dict_pk_def: TableDefinition<'static, K, V>, value_to_dict_pk_def: TableDefinition<'static, V, K>, dict_pk_by_id_def: TableDefinition<'static, K, K>) -> Self {
        Self {
            dict_pk_to_ids_def,
            value_by_dict_pk_def,
            value_to_dict_pk_def,
            dict_pk_by_id_def,
            lru_capacity
        }
    }
}

impl<K: Key + 'static, V: Key + 'static> TableFactory<K, V> for DictFactory<K, V> {
    type CacheCtx = LruCache<Vec<u8>, Vec<u8>>;
    type Table<'txn, 'c> = DictTable<'txn, 'c, K, V>;

    fn new_cache(&self) -> Self::CacheCtx {
        LruCache::new(NonZeroUsize::new(self.lru_capacity).expect("lru_capacity for dictionary must be > 0"))
    }

    fn open<'txn, 'c>(
        &self,
        tx: &'txn WriteTransaction,
        cache: &'c mut Self::CacheCtx,
    ) -> Result<Self::Table<'txn, 'c>, AppError> {
        DictTable::new(
            tx,
            cache,
            self.dict_pk_to_ids_def,
            self.value_by_dict_pk_def,
            self.value_to_dict_pk_def,
            self.dict_pk_by_id_def,
        )
    }
}
pub struct DictTable<'txn, 'c, K: Key + 'static, V: Key + 'static> {
    pub(crate) dict_pk_to_ids: MultimapTable<'txn, K, K>,
    pub(crate) value_by_dict_pk: Table<'txn, K, V>,
    pub(crate) value_to_dict_pk: Table<'txn, V, K>,
    pub(crate) dict_pk_by_id: Table<'txn, K, K>,
    cache: &'c mut LruCache<Vec<u8>, Vec<u8>>,
}

impl<'txn, 'c, K: Key + 'static, V: Key + 'static> DictTable<'txn, 'c, K, V> {
    pub fn new(
        write_tx: &'txn WriteTransaction,
        cache: &'c mut LruCache<Vec<u8>, Vec<u8>>,
        dict_pk_to_ids_def: MultimapTableDefinition<K, K>,
        value_by_dict_pk_def: TableDefinition<K, V>,
        value_to_dict_pk_def: TableDefinition<V, K>,
        dict_pk_by_id_def: TableDefinition<K, K>,
    ) -> Result<Self, AppError> {
        Ok(Self {
            dict_pk_to_ids: write_tx.open_multimap_table(dict_pk_to_ids_def)?,
            value_by_dict_pk: write_tx.open_table(value_by_dict_pk_def)?,
            value_to_dict_pk: write_tx.open_table(value_to_dict_pk_def)?,
            dict_pk_by_id: write_tx.open_table(dict_pk_by_id_def)?,
            cache,
        })
    }
}
impl<'txn, 'c, K: Key + 'static, V: Key + 'static> WriteTableLike<K, V> for DictTable<'txn, 'c, K, V> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<(), AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        let val_ref: &V::SelfType<'v> = value.borrow();

        let v_bytes_key = V::as_bytes(val_ref);
        if let Some(k_bytes) = self.cache.get(v_bytes_key.as_ref()) {
            let birth_id = K::from_bytes(k_bytes);
            self.dict_pk_by_id.insert(key_ref, &birth_id)?;
            self.dict_pk_to_ids.insert(birth_id, key_ref)?;
            Ok(())
        } else {
            if let Some(birth_id_guard) = self.value_to_dict_pk.get(val_ref)? {
                let birth_id = birth_id_guard.value();
                let v_bytes = v_bytes_key.as_ref().to_vec();
                let k_bytes = K::as_bytes(&birth_id).as_ref().to_vec();
                self.cache.put(v_bytes, k_bytes);

                self.dict_pk_by_id.insert(key_ref, &birth_id)?;
                self.dict_pk_to_ids.insert(birth_id, key_ref)?;
            } else {
                self.value_to_dict_pk.insert(val_ref, key_ref)?;
                self.value_by_dict_pk.insert(key_ref, val_ref)?;
                self.dict_pk_by_id.insert(key_ref, key_ref)?;
                self.dict_pk_to_ids.insert(key_ref, key_ref)?;

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
            let was_removed = self.dict_pk_to_ids.remove(&birth_id, key_ref)?;
            if self.dict_pk_to_ids.get(&birth_id)?.is_empty() && let Some(value_guard) = self.value_by_dict_pk.remove(&birth_id)? {
                let value = value_guard.value();
                self.value_to_dict_pk.remove(&value)?;

                // evict from cache (value -> dict_pk)
                let v_bytes = V::as_bytes(&value).as_ref().to_vec();
                let _ = self.cache.pop(&v_bytes);
            }
            Ok(was_removed)
        } else {
            Ok(false)
        }
    }

    fn get_any_for_index<'v>(&mut self, _value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueBuf<K>>, AppError>  {
        unimplemented!()
    }

    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, _range: impl RangeBounds<KR> + 'a) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError> {
        unimplemented!()
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use redb::{MultimapTable, ReadableMultimapTable, ReadableTableMetadata};
    use crate::storage::dict_test_utils::*;

    #[tokio::test]
    async fn dict_table_dedups_same_value_for_two_ids() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
        let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);

        dict_insert(&mut dict, 1, &[0xaa, 0xbb, 0xcc]);
        dict_insert(&mut dict, 2, &[0xaa, 0xbb, 0xcc]);

        assert_same_birth(&dict, 1, 2, true);

        let b = birth_id_of(&dict, 1);
        assert_eq!(value_of_birth(&dict, b), vec![0xaa, 0xbb, 0xcc], "stored value mismatch");
        assert_eq!(reverse_birth_of(&dict, &[0xaa, 0xbb, 0xcc]), b, "reverse map mismatch");
        assert_eq!(cache.len(), 1, "cache should contain one entry for the value");
    }

    #[tokio::test]
    async fn dict_table_distinct_values_produce_distinct_birth_ids() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
        let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);

        dict_insert(&mut dict, 10, &[0x01, 0x02]);
        dict_insert(&mut dict, 11, &[0x01, 0x03]);

        assert_same_birth(&dict, 10, 11, false);

        let b10 = birth_id_of(&dict, 10);
        let b11 = birth_id_of(&dict, 11);
        assert_eq!(value_of_birth(&dict, b10), vec![0x01, 0x02]);
        assert_eq!(value_of_birth(&dict, b11), vec![0x01, 0x03]);

        assert_eq!(reverse_birth_of(&dict, &[0x01, 0x02]), b10);
        assert_eq!(reverse_birth_of(&dict, &[0x01, 0x03]), b11);
    }

    #[tokio::test]
    async fn dict_table_cache_reuse_on_duplicate_value() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);

        assert_eq!(cache.len(), 0, "cache starts empty");
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict_insert(&mut dict, 100, &[0xde, 0xad, 0xbe, 0xef]);
        }
        let after_first = cache.len();
        assert_eq!(after_first, 1, "cache should hold (value → birth_id) after first insert");

        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict_insert(&mut dict, 101, &[0xde, 0xad, 0xbe, 0xef]);
            assert_same_birth(&dict, 100, 101, true);
        }
        assert_eq!(cache.len(), after_first, "cache should not grow for duplicate value");
    }

    #[tokio::test]
    async fn dict_table_birth_id_is_first_inserter() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);

            dict_insert(&mut dict, 42, &[1,2,3]);        // first time this value appears
            dict_insert(&mut dict, 1000, &[1,2,3]);      // duplicate with a different id

            let b42 = birth_id_of(&dict, 42);
            let b1000 = birth_id_of(&dict, 1000);
            assert_eq!(b42, 42, "birth id should equal first seen external id");
            assert_eq!(b42, b1000, "both ids should share the same birth id (the first inserter)");
        }
    }

    #[tokio::test]
    async fn dict_table_order_independence_many_ids_same_value() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);

            // non-monotonic insertion order
            let ids = [7u32, 1, 9, 3, 2, 5, 4, 6, 8];
            for id in ids {
                dict_insert(&mut dict, id, &[0xab, 0xcd]);
            }

            let birth = birth_id_of(&dict, 7); // first inserted among these
            for id in ids {
                assert_eq!(birth_id_of(&dict, id), birth, "all ids must share the same birth id");
            }
            assert_eq!(birth, 7, "birth id must be the first inserter");
            assert_eq!(value_of_birth(&dict, birth), vec![0xab, 0xcd]);
        }
        assert_eq!(cache.len(), 1, "single cache entry for this value");
    }

    #[tokio::test]
    async fn dict_table_idempotent_same_id_same_value() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);

            dict_insert(&mut dict, 500, &[0x11, 0x22]);
            let b1 = birth_id_of(&dict, 500);

            // repeat exact insert; should be a no-op semantically
            dict_insert(&mut dict, 500, &[0x11, 0x22]);
            let b2 = birth_id_of(&dict, 500);

            assert_eq!(b1, b2, "re-inserting same (id,value) must not change birth id");
            assert_eq!(value_of_birth(&dict, b1), vec![0x11, 0x22]);
            assert_eq!(reverse_birth_of(&dict, &[0x11, 0x22]), b1);
        }
        // cache should still only have one entry for that value
        assert_eq!(cache.len(), 1);
    }

    #[tokio::test]
    async fn dict_table_table_hit_when_cache_evicted() {
        // Force eviction so the second insert must take the table path, not cache
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1);
        // phase 1: first insert populates cache
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict_insert(&mut dict, 1, &[0xaa]); // value A
            // dict drops at end of block → releases &mut cache
        }
        assert_eq!(cache.len(), 1, "cache should have one entry after first insert");

        // phase 2: evict by inserting unrelated values (B and C)
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict_insert(&mut dict, 2, &[0xbb]); // value B
            dict_insert(&mut dict, 3, &[0xcc]); // value C
        }
        assert!(cache.len() <= 1, "tiny cache must have evicted older entries");

        // phase 3: insert same value A again; cache likely misses, but table must dedup
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict_insert(&mut dict, 4, &[0xaa]); // same value A

            let b1 = birth_id_of(&dict, 1);
            let b4 = birth_id_of(&dict, 4);
            assert_eq!(b1, b4, "table reverse index must dedup even without cache hit");
            assert_eq!(value_of_birth(&dict, b1), vec![0xaa]);
        }
    }

    #[tokio::test]
    async fn dict_table_zero_length_and_long_values_roundtrip() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);

            // zero-length
            dict_insert(&mut dict, 10, &[]);
            dict_insert(&mut dict, 11, &[]);
            assert_same_birth(&dict, 10, 11, true);
            let b0 = birth_id_of(&dict, 10);
            assert_eq!(value_of_birth(&dict, b0), Vec::<u8>::new());

            // long value
            let big = vec![0u8; 4096];
            dict_insert(&mut dict, 20, &big);
            dict_insert(&mut dict, 21, &big);
            assert_same_birth(&dict, 20, 21, true);
            let b_big = birth_id_of(&dict, 20);
            assert_eq!(value_of_birth(&dict, b_big), big);
        }
    }

    #[tokio::test]
    async fn dict_table_many_unique_values_have_unique_birth_ids() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);

            // Insert N unique values; ensure bijection id -> birth id and reverse map exists
            let n = 100usize;
            let mut births = std::collections::BTreeSet::new();
            for i in 0..n {
                let id = i as u32 + 1000;
                let v = vec![i as u8, (i * 7) as u8];
                dict_insert(&mut dict, id, &v);
                let b = birth_id_of(&dict, id);
                births.insert(b);
                assert_eq!(value_of_birth(&dict, b), v);
                assert_eq!(reverse_birth_of(&dict, &v), b);
            }
            assert_eq!(births.len(), n, "each unique value must yield a unique birth id");
        }
    }

    #[tokio::test]
    async fn multimap_len_semantics_pairs_vs_keys() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);

        // helpers
        fn distinct_keys_len(mm: &MultimapTable<'_, u32, u32>) -> usize {
            let mut n = 0usize;
            let mut it = mm.iter().expect("iter()");
            while let Some(kv) = it.next() {
                let (_k, mut vals) = kv.expect("key");
                n += 1;                         // one per distinct key
                while let Some(v) = vals.next() { v.expect("val"); } // drain values
            }
            n
        }
        fn total_pairs_len(mm: &MultimapTable<'_, u32, u32>) -> usize {
            let mut n = 0usize;
            let mut it = mm.iter().expect("iter()");
            while let Some(kv) = it.next() {
                let (_k, mut vals) = kv.expect("key");
                while let Some(v) = vals.next() { v.expect("val"); n += 1; }
            }
            n
        }

        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);

            dict_insert(&mut dict, 1, &[0xaa]); // birth 1
            dict_insert(&mut dict, 2, &[0xaa]); // duplicate of 1
            dict_insert(&mut dict, 3, &[0xbb]); // birth 3

            // what the structure actually is
            let distinct = distinct_keys_len(&dict.dict_pk_to_ids); // expect 2
            let pairs    = total_pairs_len(&dict.dict_pk_to_ids);   // expect 3
            assert_eq!(distinct, 2, "distinct birth ids");
            assert_eq!(pairs, 3, "key→value pairs");

            // tolerate either len() contract (keys or pairs)
            let mm_len = dict.dict_pk_to_ids.len().expect("len()") as usize;
            assert!(mm_len == distinct || mm_len == pairs,
                    "len()={} should equal distinct keys ({}) or total pairs ({})", mm_len, distinct, pairs);
        }
    }
}

