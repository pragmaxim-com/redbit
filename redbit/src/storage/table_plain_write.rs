use crate::storage::table_writer::{TableFactory, WriteTableLike};
use crate::AppError;
use redb::*;
use redb::{Key, Table, WriteTransaction};
use std::borrow::Borrow;
use std::ops::RangeBounds;
use crate::storage::async_boundary::{CopyOwnedValue, ValueBuf, ValueOwned};

#[derive(Clone)]
pub struct PlainFactory<K: Key + 'static, V: Key + 'static> {
    pub(crate) name: String,
    pub(crate) table_def: TableDefinition<'static, K, V>,
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
    pub fn new(write_tx: &'txn WriteTransaction, table_def: TableDefinition<'static, K, V>) -> Result<Self, AppError> {
        Ok(Self {
            table: write_tx.open_table(table_def)?,
        })
    }
}

impl<K: Key + CopyOwnedValue + 'static, V: Key + 'static> TableFactory<K, V> for PlainFactory<K, V> {
    type CacheCtx = ();
    type Table<'txn, 'c> = PlainTable<'txn, K, V>;

    fn new_cache(&self) -> Self::CacheCtx { }


    fn name(&self) -> String {
        self.name.clone()
    }

    fn open<'txn, 'c>(&self, tx: &'txn WriteTransaction, _cache: &'c mut Self::CacheCtx) -> Result<Self::Table<'txn, 'c>, AppError> {
        PlainTable::new(tx, self.table_def)
    }
}

impl<'txn, K: Key + CopyOwnedValue + 'static, V: Key + 'static> WriteTableLike<K, V> for PlainTable<'txn, K, V> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<(), AppError>  {
        self.table.insert(key, value)?;
        Ok(())
    }

    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> Result<bool, AppError>  {
        let removed = self.table.remove(key)?;
        Ok(removed.is_some())
    }

    fn get_any_for_index<'v>(&mut self, _value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueOwned<K>>, AppError>  {
        unimplemented!()
    }

    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, range: impl RangeBounds<KR> + 'a) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError> {
        let mut result: Vec<(ValueBuf<K>, ValueBuf<V>)> = Vec::new();
        let mm = self.table.range(range);
        for tuple in mm? {
            let (k_guard, v_guard) = tuple?;
            result.push((Self::key_buf(k_guard), Self::value_buf(v_guard)));
        }
        Ok(result)
    }
}


#[cfg(all(test, not(feature = "integration")))]
mod plain_write_table_tests {
    use super::*;
    use crate::WriteTableLike;
    use crate::storage::test_utils::{addr, Address};
    use crate::storage::plain_test_utils::{setup_plain_defs, mk_plain};

    // Empty table: any nontrivial range yields empty vec
    #[test]
    fn range_on_empty_returns_empty() {
        let (_db, tx, underlying_def) = setup_plain_defs();
        let tbl: PlainTable<'_, u32, Address> = mk_plain(&tx, underlying_def);

        let out = tbl.range(10u32..20u32).expect("range");
        assert!(out.is_empty(), "range over empty table must be empty");

        let out2 = tbl.range(0u32..=0u32).expect("range");
        assert!(out2.is_empty(), "single-point inclusive range over empty table must be empty");
    }

    // Insert ascending keys and verify range semantics + ordering.
    #[test]
    fn range_inclusive_exclusive_and_ordering() {
        let (_db, tx, underlying_def) = setup_plain_defs();
        let mut tbl: PlainTable<'_, u32, Address> = mk_plain(&tx, underlying_def);

        // Insert keys 1..=8 with small values
        for k in 1u32..=8 {
            let bytes = [k as u8, (k + 1) as u8];
            tbl.insert_kv(&k, &addr(&bytes)).expect("insert");
        }

        // Exclusive upper bound: 3..7 -> {3,4,5,6}
        let v = tbl.range(3u32..7u32).expect("range 3..7");
        let keys: Vec<u32> = v.iter()
            .map(|(kb, _vb)| <u32 as redb::Value>::from_bytes(kb.as_bytes()))
            .collect();
        assert_eq!(keys, vec![3,4,5,6], "exclusive upper bound failed");

        // Inclusive upper bound: 3..=7 -> {3,4,5,6,7}
        let v = tbl.range(3u32..=7u32).expect("range 3..=7");
        let keys: Vec<u32> = v.iter()
            .map(|(kb, _vb)| <u32 as redb::Value>::from_bytes(kb.as_bytes()))
            .collect();
        assert_eq!(keys, vec![3,4,5,6,7], "inclusive upper bound failed");

        // Single-point inclusive: 5..=5 -> {5}
        let v = tbl.range(5u32..=5u32).expect("range 5..=5");
        let keys: Vec<u32> = v.iter()
            .map(|(kb, _vb)| <u32 as redb::Value>::from_bytes(kb.as_bytes()))
            .collect();
        assert_eq!(keys, vec![5], "single-point inclusive failed");

        // Empty half-open: 5..5 -> {}
        let v = tbl.range(5u32..5u32).expect("range 5..5");
        assert!(v.is_empty(), "half-open with equal bounds must be empty");

        // Verify values align with keys we inserted
        let v_all = tbl.range(1u32..=8u32).expect("range full");
        for (kb, vb) in v_all {
            let k = <u32 as redb::Value>::from_bytes(kb.as_bytes());
            let expected = [k as u8, (k + 1) as u8];
            assert_eq!(vb.as_bytes(), expected, "value mismatch for key {}", k);
        }
    }

    // Deleting affects subsequent ranges; deleting absent key is false.
    #[test]
    fn delete_updates_range_and_idempotent() {
        let (_db, tx, underlying_def) = setup_plain_defs();
        let mut tbl: PlainTable<'_, u32, Address> = mk_plain(&tx, underlying_def);

        for k in 1u32..=6 {
            let bytes = [k as u8, (k + 1) as u8];
            tbl.insert_kv(&k, &addr(&bytes)).expect("insert");
        }

        // Delete middle key
        assert!(tbl.delete_kv(&3u32).expect("delete 3"));

        // Range around the hole: 2..5 -> {2,4}
        let v = tbl.range(2u32..5u32).expect("range 2..5");
        let keys: Vec<u32> = v.iter()
            .map(|(kb, _vb)| <u32 as redb::Value>::from_bytes(kb.as_bytes()))
            .collect();
        assert_eq!(keys, vec![2,4], "range should skip deleted key");

        // Deleting again is false
        assert!(!tbl.delete_kv(&3u32).expect("delete 3 again should be false"));

        // Delete edge key and check boundary behavior
        assert!(tbl.delete_kv(&2u32).expect("delete 2"));
        let v = tbl.range(2u32..=4u32).expect("range 2..=4");
        let keys: Vec<u32> = v.iter()
            .map(|(kb, _vb)| <u32 as redb::Value>::from_bytes(kb.as_bytes()))
            .collect();
        assert_eq!(keys, vec![4], "only 4 should remain in [2,4]");
    }
}
