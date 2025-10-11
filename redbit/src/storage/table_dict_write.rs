use crate::storage::async_boundary::{CopyOwnedValue, ValueBuf, ValueOwned};
use crate::storage::table_writer_api::{TableFactory, WriteTableLike};
use crate::{AppError, CacheKey};
use lru::LruCache;
use redb::*;
use redb::{Table, WriteTransaction};
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::num::NonZeroUsize;
use std::ops::RangeBounds;

#[derive(Clone)]
pub struct DictFactory<K: CopyOwnedValue + 'static, V: CacheKey + 'static> {
    pub name: String,
    pub dict_pk_to_ids_def: MultimapTableDefinition<'static, K, K>,
    pub value_by_dict_pk_def: TableDefinition<'static, K, V>,
    pub value_to_dict_pk_def: TableDefinition<'static, V, K>,
    pub dict_pk_by_id_def: TableDefinition<'static, K, K>,
    pub lru_capacity: usize,
}

impl<K: CopyOwnedValue + 'static, V: CacheKey + 'static> DictFactory<K, V> {
    pub fn new( name: &str, lru_capacity: usize, dict_pk_to_ids_def: MultimapTableDefinition<'static, K, K>, value_by_dict_pk_def: TableDefinition<'static, K, V>, value_to_dict_pk_def: TableDefinition<'static, V, K>, dict_pk_by_id_def: TableDefinition<'static, K, K>) -> Self {
        Self {
            name: name.to_string(),
            dict_pk_to_ids_def,
            value_by_dict_pk_def,
            value_to_dict_pk_def,
            dict_pk_by_id_def,
            lru_capacity
        }
    }
}

impl<K: CopyOwnedValue + 'static, V: CacheKey + 'static> TableFactory<K, V> for DictFactory<K, V> {
    type CacheCtx = LruCache<V::CK, K::Unit>;
    type Table<'txn, 'c> = DictTable<'txn, 'c, K, V>;

    fn name(&self) -> String {
        self.name.clone()
    }

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
pub struct DictTable<'txn, 'c, K: CopyOwnedValue + 'static, V: CacheKey + 'static> {
    pub(crate) dict_pk_to_keys: MultimapTable<'txn, K, K>,
    pub(crate) value_by_dict_pk: Table<'txn, K, V>,
    pub(crate) value_to_dict_pk: Table<'txn, V, K>,
    pub(crate) dict_pk_by_key: Table<'txn, K, K>,
    cache: &'c mut LruCache<V::CK, K::Unit>,
}

impl<'txn, 'c, K: CopyOwnedValue + 'static, V: CacheKey + 'static> DictTable<'txn, 'c, K, V> {
    pub fn new(
        write_tx: &'txn WriteTransaction,
        cache: &'c mut LruCache<V::CK, K::Unit>,
        dict_pk_to_ids_def: MultimapTableDefinition<K, K>,
        value_by_dict_pk_def: TableDefinition<K, V>,
        value_to_dict_pk_def: TableDefinition<V, K>,
        dict_pk_by_id_def: TableDefinition<K, K>,
    ) -> Result<Self, AppError> {
        Ok(Self {
            dict_pk_to_keys: write_tx.open_multimap_table(dict_pk_to_ids_def)?,
            value_by_dict_pk: write_tx.open_table(value_by_dict_pk_def)?,
            value_to_dict_pk: write_tx.open_table(value_to_dict_pk_def)?,
            dict_pk_by_key: write_tx.open_table(dict_pk_by_id_def)?,
            cache,
        })
    }
}
impl<'txn, 'c, K: CopyOwnedValue + 'static, V: CacheKey + 'static> WriteTableLike<K, V> for DictTable<'txn, 'c, K, V> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<(), AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        let val_ref: &V::SelfType<'v> = value.borrow();
        let cache_key: V::CK = V::cache_key(val_ref);

        if let Some(&k) = self.cache.get(&cache_key) {
            let birth_id = Self::owned_from_unit(k);
            self.dict_pk_by_key.insert(key_ref, birth_id.as_value())?;
            self.dict_pk_to_keys.insert(birth_id.as_value(), key_ref)?;
            Ok(())
        } else {
            if let Some(birth_id_guard) = self.value_to_dict_pk.get(val_ref)? {
                let birth_id = Self::owned_key_from_guard(birth_id_guard);
                self.cache.put(cache_key, birth_id.into_unit());

                self.dict_pk_by_key.insert(key_ref, birth_id.as_value())?;
                self.dict_pk_to_keys.insert(birth_id.as_value(), key_ref)?;
            } else {
                self.value_to_dict_pk.insert(val_ref, key_ref)?;
                self.value_by_dict_pk.insert(key_ref, val_ref)?;
                self.dict_pk_by_key.insert(key_ref, key_ref)?;
                self.dict_pk_to_keys.insert(key_ref, key_ref)?;

                self.cache.put(cache_key, Self::unit_from_key(key_ref));
            }
            Ok(())
        }
    }

    fn insert_many_kvs<'k, 'v, KR: Borrow<K::SelfType<'k>>, VR: Borrow<V::SelfType<'v>>>(
        &mut self,
        mut pairs: Vec<(KR, VR)>,
        sort_by_key: bool,
    ) -> Result<(), AppError> {
        // --- Optional Run 0: sort by Key for locality in key-keyed tables ---
        if sort_by_key {
            pairs.sort_by(|(a, _), (b, _)| {
                let a_bytes = K::as_bytes(a.borrow());
                let b_bytes = K::as_bytes(b.borrow());
                K::compare(a_bytes.as_ref(), b_bytes.as_ref())
            });
        } else {
            for w in pairs.windows(2) {
                let (ka, _) = &w[0];
                let (kb, _) = &w[1];
                let ord = K::compare(
                    K::as_bytes(ka.borrow()).as_ref(),
                    K::as_bytes(kb.borrow()).as_ref(),
                );
                assert!(
                    matches!(ord, Ordering::Less | Ordering::Equal),
                    "insert_many_kvs(sort_by_key=false): input must be sorted by key"
                );
            }
        }

        // We defer writes that are *not* keyed by the input Key:
        // - dict_pk_to_keys: (birth_id -> key)  => sort by birth_id
        // - value_to_dict_pk: (value -> birth_id) => sort by value
        //
        // NOTE: keep these unannotated; let the compiler infer the owned key types returned
        // by helpers like `owned_from_unit(..)` / `owned_key_from_guard(..)`.
        let mut pk_to_keys_batch = Vec::with_capacity(pairs.len());
        let mut value_to_pk_batch = Vec::new();

        // --- Run 1: linear pass in Key order, update cache early ---
        for (k, v) in &pairs {
            let key_ref: &K::SelfType<'k> = k.borrow();
            let val_ref: &V::SelfType<'v> = v.borrow();
            let cache_key: V::CK = V::cache_key(val_ref);

            if let Some(&unit) = self.cache.get(&cache_key) {
                // Existing value found via cache → we know birth_id.
                let birth_id = Self::owned_from_unit(unit);
                self.dict_pk_by_key.insert(key_ref, birth_id.as_value())?;

                // Defer (birth_id -> key) write; we’ll sort by birth_id in Run 2.
                let key_owned = Self::owned_from_unit(Self::unit_from_key(key_ref));
                pk_to_keys_batch.push((birth_id, key_owned));
            } else if let Some(birth_id_guard) = self.value_to_dict_pk.get(val_ref)? {
                // Existing value found in table → materialize, seed cache, same as above.
                let birth_id = Self::owned_key_from_guard(birth_id_guard);
                self.cache.put(cache_key, birth_id.into_unit());

                self.dict_pk_by_key.insert(key_ref, birth_id.as_value())?;

                let key_owned = Self::owned_from_unit(Self::unit_from_key(key_ref));
                pk_to_keys_batch.push((birth_id, key_owned));
            } else {
                // Brand-new value. The first key we encounter becomes the birth_id.
                // Only write tables keyed by `key` now; defer the rest.
                self.dict_pk_by_key.insert(key_ref, key_ref)?;
                self.value_by_dict_pk.insert(key_ref, val_ref)?;

                // Prepare deferred writes:
                let birth_id_owned = Self::owned_from_unit(Self::unit_from_key(key_ref));
                // (birth_id -> key)
                pk_to_keys_batch.push((birth_id_owned.clone(), birth_id_owned.clone()));
                // (value -> birth_id)
                value_to_pk_batch.push((val_ref, birth_id_owned.clone()));

                // Seed cache so subsequent keys with the same value in this batch hit fast path.
                self.cache.put(cache_key, Self::unit_from_key(key_ref));
            }
        }

        // --- Run 2a: flush dict_pk_to_keys in birth_id order ---
        pk_to_keys_batch.sort_by(|(a, _), (b, _)| {
            let aav = &a.as_value();
            let abv = &b.as_value();
            K::compare(K::as_bytes(aav).as_ref(), K::as_bytes(abv).as_ref())
        });
        for (birth_id_owned, key_owned) in pk_to_keys_batch {
            self.dict_pk_to_keys.insert(birth_id_owned.as_value(), key_owned.as_value())?;
        }

        // --- Run 2b: flush value_to_dict_pk in value order ---
        value_to_pk_batch.sort_by(|a, b| {
            V::compare(V::as_bytes(a.0).as_ref(), V::as_bytes(b.0).as_ref())
        });
        for (val_ref, birth_id_owned) in value_to_pk_batch {
            self.value_to_dict_pk.insert(val_ref, birth_id_owned.as_value())?;
        }

        Ok(())
    }

    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> Result<bool, AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        if let Some(birth_guard) = self.dict_pk_by_key.remove(key_ref)? {
            let birth_id = birth_guard.value();
            let was_removed = self.dict_pk_to_keys.remove(&birth_id, key_ref)?;
            if self.dict_pk_to_keys.get(&birth_id)?.is_empty() && let Some(value_guard) = self.value_by_dict_pk.remove(&birth_id)? {
                let value = value_guard.value();
                self.value_to_dict_pk.remove(&value)?;
                let _ = self.cache.pop(&V::cache_key(&value));
            }
            Ok(was_removed)
        } else {
            Ok(false)
        }
    }

    fn get_any_for_index<'v>(&mut self, _value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueOwned<K>>, AppError>  {
        unimplemented!()
    }

    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, _range: impl RangeBounds<KR> + 'a) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError> {
        unimplemented!()
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use crate::storage::dict_test_utils::*;
    use redb::{MultimapTable, ReadableMultimapTable, ReadableTable, ReadableTableMetadata};
    use crate::DictTable;
    use crate::storage::table_writer_api::WriteTableLike;
    use crate::storage::test_utils::{addr, Address};

    /// Read the birth id for a given external id.
    pub(crate) fn birth_id_of(dict: &DictTable<'_, '_, u32, Address>, id: u32) -> u32 {
        dict.dict_pk_by_key.get(&id).expect("get").expect("missing").value()
    }

    /// Read the stored value under a birth id.
    pub(crate) fn value_of_birth(dict: &DictTable<'_, '_, u32, Address>, b: u32) -> Vec<u8> {
        dict.value_by_dict_pk.get(&b).expect("get").expect("missing").value().0
    }

    /// Reverse lookup birth id from a value (value_to_dict_pk).
    pub(crate) fn reverse_birth_of(dict: &DictTable<'_, '_, u32, Address>, v: &[u8]) -> u32 {
        dict.value_to_dict_pk.get(&addr(v)).expect("get").expect("missing").value()
    }

    /// Assert two ids share (or don’t share) birth ids.
    pub(crate) fn assert_same_birth(dict: &DictTable<'_, '_, u32, Address>, lhs: u32, rhs: u32, expect_same: bool) {
        let bl = birth_id_of(dict, lhs);
        let br = birth_id_of(dict, rhs);
        if expect_same { assert_eq!(bl, br, "ids {lhs} and {rhs} should share a birth id"); } else { assert_ne!(bl, br, "ids {lhs} and {rhs} should not share a birth id"); }
    }

    #[tokio::test]
    async fn dict_table_dedups_same_value_for_two_ids() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
        let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);

        dict.insert_kv(1, addr(&[0xaa, 0xbb, 0xcc])).expect("insert");
        dict.insert_kv(2, addr(&[0xaa, 0xbb, 0xcc])).expect("insert");

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

        dict.insert_kv( 10, addr(&[0x01, 0x02])).expect("insert");
        dict.insert_kv(11, addr(&[0x01, 0x03])).expect("insert");

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
            dict.insert_kv( 100, addr(&[0xde, 0xad, 0xbe, 0xef])).expect("insert");
        }
        let after_first = cache.len();
        assert_eq!(after_first, 1, "cache should hold (value → birth_id) after first insert");

        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict.insert_kv(101, addr(&[0xde, 0xad, 0xbe, 0xef])).expect("insert");
            assert_same_birth(&dict, 100, 101, true);
        }
        assert_eq!(cache.len(), after_first, "cache should not grow for duplicate value");
    }

    #[tokio::test]
    async fn dict_table_birth_id_is_first_inserter() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);

            dict.insert_kv(42, addr(&[1,2,3])).expect("insert");        // first time this value appears
            dict.insert_kv(1000, addr(&[1,2,3])).expect("insert");      // duplicate with a different id

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
                dict.insert_kv(id, addr(&[0xab, 0xcd])).expect("insert");
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

            dict.insert_kv( 500, addr(&[0x11, 0x22])).expect("insert");
            let b1 = birth_id_of(&dict, 500);

            // repeat exact insert; should be a no-op semantically
            dict.insert_kv( 500, addr(&[0x11, 0x22])).expect("insert");
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
            dict.insert_kv(1, addr(&[0xaa])).expect("insert"); // value A
            // dict drops at end of block → releases &mut cache
        }
        assert_eq!(cache.len(), 1, "cache should have one entry after first insert");

        // phase 2: evict by inserting unrelated values (B and C)
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict.insert_kv(2, addr(&[0xbb])).expect("insert"); // value B
            dict.insert_kv(3, addr(&[0xcc])).expect("insert"); // value C
        }
        assert!(cache.len() <= 1, "tiny cache must have evicted older entries");

        // phase 3: insert same value A again; cache likely misses, but table must dedup
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict.insert_kv(4, addr(&[0xaa])).expect("insert"); // same value A

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
            dict.insert_kv(10, addr(&[])).expect("insert");
            dict.insert_kv(11, addr(&[])).expect("insert");
            assert_same_birth(&dict, 10, 11, true);
            let b0 = birth_id_of(&dict, 10);
            assert_eq!(value_of_birth(&dict, b0), Vec::<u8>::new());

            // long value
            let big = vec![0u8; 4096];
            dict.insert_kv(20, addr(&big)).expect("insert");
            dict.insert_kv(21, addr(&big)).expect("insert");
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
                dict.insert_kv(id, addr(&v)).expect("insert");
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
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs::<u32, Address>(1000);

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

            dict.insert_kv(1, addr(&[0xaa])).expect("insert"); // birth 1
            dict.insert_kv(2, addr(&[0xaa])).expect("insert"); // duplicate of 1
            dict.insert_kv(3, addr(&[0xbb])).expect("insert"); // birth 3

            // what the structure actually is
            let distinct = distinct_keys_len(&dict.dict_pk_to_keys); // expect 2
            let pairs    = total_pairs_len(&dict.dict_pk_to_keys);   // expect 3
            assert_eq!(distinct, 2, "distinct birth ids");
            assert_eq!(pairs, 3, "key→value pairs");

            // tolerate either len() contract (keys or pairs)
            let mm_len = dict.dict_pk_to_keys.len().expect("len()") as usize;
            assert!(mm_len == distinct || mm_len == pairs,
                    "len()={} should equal distinct keys ({}) or total pairs ({})", mm_len, distinct, pairs);
        }
    }

    #[tokio::test]
    async fn dict_table_batch_same_value_birth_depends_on_sort_flag() {
        // three ids share the same value
        let v = addr(&[0x7a, 0x7a, 0x7a]);

        // Case A: sort_by_key = false → first in vector wins as birth
        {
            let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            let pairs = vec![(9u32, &v), (3u32, &v), (5u32, &v)];
            dict.insert_many_kvs(pairs, /*sort_by_key*/ false).expect("batch");

            let b9 = birth_id_of(&dict, 9);
            let b3 = birth_id_of(&dict, 3);
            let b5 = birth_id_of(&dict, 5);
            assert_eq!(b9, 9, "first inserter in given order is birth when not sorting");
            assert_eq!(b9, b3);
            assert_eq!(b9, b5);
        }

        // Case B: sort_by_key = true → minimal key wins as birth (by K compare)
        {
            let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            let pairs = vec![(9u32, &v), (3u32, &v), (5u32, &v)];
            dict.insert_many_kvs(pairs, /*sort_by_key*/ true).expect("batch");

            let b9 = birth_id_of(&dict, 9);
            let b3 = birth_id_of(&dict, 3);
            let b5 = birth_id_of(&dict, 5);
            assert_eq!(b3, 3, "minimal key becomes birth when sorting by key");
            assert_eq!(b3, b5);
            assert_eq!(b3, b9);
        }
    }

    #[tokio::test]
    async fn dict_table_batch_mixed_existing_and_new_values() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);

        // Seed: value A is already known with birth=100
        let val_a = addr(&[0x0a, 0x0a]);
        let val_b = addr(&[0x0b, 0x0b]);
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict.insert_kv(100u32, &val_a).expect("seed A");
            assert_eq!(birth_id_of(&dict, 100), 100);
        }

        // Batch: add two more ids for A and one new value B (brand-new birth)
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            let pairs = vec![(200u32, &val_a), (201u32, &val_a), (300u32, &val_b)];
            dict.insert_many_kvs(pairs, /*sort_by_key*/ true).expect("batch");

            // A: should still map to birth 100 for any id
            for id in [100u32, 200, 201] {
                assert_eq!(birth_id_of(&dict, id), 100, "existing value must keep original birth");
            }
            assert_eq!(reverse_birth_of(&dict, &val_a.0), 100);

            // B: new birth is whichever wins by key order (here 300)
            let b_b = birth_id_of(&dict, 300);
            assert_eq!(b_b, 300);
            assert_eq!(value_of_birth(&dict, b_b), val_b.0);

            // dict_pk_to_keys for birth 100 must contain all three ids: 100,200,201
            let mut has = std::collections::BTreeSet::new();
            let mut it = dict.dict_pk_to_keys.get(&100).expect("missing");
            while let Some(v) = it.next() {
                has.insert(v.expect("v").value());
            }
            assert!(has.contains(&100) && has.contains(&200) && has.contains(&201),
                    "dict_pk_to_keys(100) must include 100,200,201");
        }
    }

    #[tokio::test]
    async fn dict_table_batch_idempotent_duplicate_pairs_in_single_batch() {
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);

        let val = addr(&[0x55, 0x66]);
        let pairs = vec![
            (42u32, &val),
            (42u32, &val), // duplicate (id,value) in the same batch
            (7u32,  &val),
            (7u32,  &val), // duplicate (id,value) in the same batch
        ];

        let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
        dict.insert_many_kvs(pairs, /*sort_by_key*/ true).expect("batch");

        // Mapping correctness
        let b42 = birth_id_of(&dict, 42);
        let b7  = birth_id_of(&dict, 7);
        assert_eq!(b42, std::cmp::min(7,42), "birth must be minimal key under sort_by_key");
        assert_eq!(b42, b7);
        assert_eq!(reverse_birth_of(&dict, &val.0), b42);

        // Ensure dict_pk_by_key holds the same birth id after duplicates
        assert_eq!(birth_id_of(&dict, 42), b42);
        assert_eq!(birth_id_of(&dict, 7),  b42);

        // Ensure value_by_dict_pk is a single copy of the value under the birth id
        assert_eq!(value_of_birth(&dict, b42), val.0);
    }

    #[tokio::test]
    async fn dict_table_batch_zero_new_value_to_pk_entries_for_existing_value() {
        // For an existing value, batch inserts must not create new value_to_dict_pk rows.
        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
        let val = addr(&[0x44]);

        // Seed existing
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict.insert_kv(10u32, &val).expect("seed");
            assert_eq!(reverse_birth_of(&dict, &val.0), 10);
        }

        // Batch adds more ids for the same value
        {
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict.insert_many_kvs(vec![(11u32, &val), (12u32, &val)], true).expect("batch");

            // Reverse map still points to original birth → no duplicate rows created
            assert_eq!(reverse_birth_of(&dict, &val.0), 10);
        }
    }

    #[tokio::test]
    async fn dict_table_batch_mixed_orders_equivalence() {
        let val = addr(&[0x33, 0x33]);

        // unsorted path
        {
            let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict.insert_many_kvs(vec![(5u32, &val), (1u32, &val), (9u32, &val)], false).expect("unsorted");
            assert_eq!(birth_id_of(&dict, 5), 5, "first in input is birth for unsorted");
            assert_eq!(reverse_birth_of(&dict, &val.0), 5);
        }

        // sorted path
        {
            let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
            let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);
            dict.insert_many_kvs(vec![(5u32, &val), (1u32, &val), (9u32, &val)], true).expect("sorted");
            assert_eq!(birth_id_of(&dict, 1), 1, "min key is birth for sorted");
            assert_eq!(reverse_birth_of(&dict, &val.0), 1);
        }
    }

    #[tokio::test]
    async fn dict_table_batch_table_invariants_hold() {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let (_db, tx, mut cache, t1, t2, t3, t4) = setup_dict_defs(1000);
        let mut dict = mk_dict(&tx, &mut cache, t1, t2, t3, t4);

        // build a batch with duplicates and uniques
        let mut vals = Vec::new();
        for i in 0..50u32 {
            let tag: u8 = rng.random_range(0..6); // 6 “value groups”
            let v = addr(&[tag, (i % 17) as u8]);
            vals.push((1000 + i, v));
        }
        // take refs
        let pairs: Vec<(u32, &Address)> = vals.iter().map(|(k,v)| (*k, v)).collect();
        dict.insert_many_kvs(pairs, true).expect("batch");

        // invariant 1: for every key k, dict_pk_by_key(k)=b implies:
        //  - value_by_dict_pk(b) exists
        //  - dict_pk_to_keys(b) contains k
        let mut it = dict.dict_pk_by_key.iter().expect("iter");
        while let Some(kv) = it.next() {
            let (k, b) = kv.expect("kv");
            let k = k.value();
            let b = b.value();
            let v_guard = dict.value_by_dict_pk.get(&b).expect("get");
            assert!(v_guard.is_some(), "value_by_dict_pk missing for birth {}", b);

            let mut keys_it = dict.dict_pk_to_keys.get(&b).expect("missing");
            let mut found = false;
            while let Some(v) = keys_it.next() {
                if v.expect("v").value() == k { found = true; break; }
            }
            assert!(found, "dict_pk_to_keys({}) does not contain {}", b, k);
        }

        // invariant 2: reverse map consistent
        let mut it = dict.value_by_dict_pk.iter().expect("iter");
        while let Some(kv) = it.next() {
            let (b, v) = kv.expect("kv");
            let b = b.value();
            let Address(vbytes) = v.value();
            let rb = reverse_birth_of(&dict, &vbytes);
            assert_eq!(rb, b, "reverse map mismatch for birth {}", b);
        }
    }

}

