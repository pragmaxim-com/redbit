pub mod table_dict_read;
pub mod table_dict_write;
pub mod table_index_read;
pub mod table_index_write;
pub mod table_plain_write;
pub mod table_writer;
pub mod cache;
pub mod table_writer_sharded;
pub mod partitioning;
pub mod table_dict_read_sharded;
pub mod table_index_read_sharded;
pub mod table_plain_read;
pub mod table_plain_read_sharded;
pub mod context;
pub mod init;
pub mod bench;
pub mod async_boundary;

#[cfg(all(test, not(feature = "integration")))]
pub mod test_utils {
    use redb::{Database, Key, TypeName, Value};
    use std::cmp::Ordering;
    use std::sync::{Arc, Weak};
    use crate::CacheKey;

    #[derive(Debug, Clone)]
    pub(crate)  struct Address(pub Vec<u8>);
    impl Value for Address {
        type SelfType<'a> = Address
        where
            Self: 'a;
        type AsBytes<'a> = &'a [u8]
        where
            Self: 'a;
        fn fixed_width() -> Option<usize> {
            None
        }
        fn from_bytes<'a>(data: &'a [u8]) -> Address
        where
            Self: 'a,
        {
            Address(data.to_vec())
        }
        fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> &'a [u8]
        where
            Self: 'a,
            Self: 'b,
        {
            value.0.as_ref()
        }
        fn type_name() -> TypeName {
            TypeName::new("Vec<u8>")
        }
    }
    impl Key for Address {
        fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
            data1.cmp(data2)
        }
    }
    impl CacheKey for Address {
        type CK = Vec<u8>;

        #[inline]
        fn cache_key<'a>(v: &<Address as Value>::SelfType<'a>) -> Self::CK
        where
            Address: 'a,
        {
            v.0.clone()
        }
    }

    pub(crate) fn addr(bytes: &[u8]) -> Address { Address(bytes.to_vec()) }

    pub(crate) fn mk_db(prefix: &str) -> (Arc<Database>, Weak<Database>) {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, rand::random::<u64>()));
        let db = Database::builder().create(path).expect("create db");
        let owned = Arc::new(db);
        let weak = Arc::downgrade(&owned);
        (owned, weak)
    }

    pub(crate) fn mk_shard_dbs(n: usize, prefix: &str) -> (Vec<Arc<Database>>, Vec<Weak<Database>>) {
        assert!(n >= 2);
        let mut owned = Vec::with_capacity(n);
        for i in 0..n {
            let path = std::env::temp_dir().join(format!("{}_{}_{}", prefix, i, rand::random::<u64>()));
            let db = Database::builder().create(path).expect("create db");
            owned.push(Arc::new(db));
        }
        let weak = owned.iter().map(Arc::downgrade).collect::<Vec<_>>();
        (owned, weak)
    }
}

#[cfg(all(test, not(feature = "integration")))]
pub mod plain_test_utils {
    use crate::storage::table_plain_write::PlainTable;
    use crate::storage::test_utils::Address;
    use crate::*;
    use redb::{Database, TableDefinition, WriteTransaction};

    pub(crate) fn mk_sharded_reader(n: usize, weak_dbs: Vec<Weak<Database>>, plain_def: TableDefinition<'static, u32, Address>) -> ShardedReadOnlyPlainTable<u32, Address, BytesPartitioner> {
        ShardedReadOnlyPlainTable::new(
            BytesPartitioner::new(n),
            weak_dbs.clone(),
            plain_def,
        ).expect("reader")
    }

    pub(crate) fn mk_sharded_writer(name: &str, n: usize, weak_dbs: Vec<Weak<Database>>) -> (ShardedTableWriter<u32, Address, PlainFactory<u32, Address>, BytesPartitioner, Xxh3Partitioner>, TableDefinition<'static, u32, Address>) {
        let plain_def = TableDefinition::<u32, Address>::new("plain_underlying");

        // Writer/Reader
        let writer = ShardedTableWriter::new(
            Partitioning::by_key(n),
            weak_dbs.clone(),
            PlainFactory::new(name, plain_def),
        ).expect("writer");

        (writer, plain_def)
    }

    pub(crate) fn setup_plain_defs() -> (
        Database,
        WriteTransaction,
        TableDefinition<'static, u32, Address>,
    ) {
        let path = std::env::temp_dir().join(format!("redbit_plain_test_{}", rand::random::<u64>()));
        let db   = Database::builder().create(path).expect("create db");
        let tx   = db.begin_write().expect("begin write");
        let tbl  = TableDefinition::<u32, Address>::new("plain_underlying");
        (db, tx, tbl)
    }

    pub(crate) fn mk_plain<'txn>(
        tx: &'txn WriteTransaction,
        underlying_def: TableDefinition<'static, u32, Address>,
    ) -> PlainTable<'txn, u32, Address> {
        // Construct directly; mirrors your struct shape.
        PlainTable {
            table: tx.open_table(underlying_def).expect("open plain table"),
        }
    }
}

#[cfg(all(test, not(feature = "integration")))]
pub mod index_test_utils {
    use crate::storage::test_utils;
    use crate::storage::test_utils::Address;
    use crate::*;
    use lru::LruCache;
    use redb::{
        Database, MultimapTableDefinition, TableDefinition,
        WriteTransaction,
    };
    use std::num::NonZeroUsize;

    pub(crate) fn mk_sharded_reader(n: usize, weak_dbs: Vec<Weak<Database>>, pk_by_index_def: MultimapTableDefinition<'static, Address, u32>, index_by_pk_def: TableDefinition<'static, u32, Address>) -> ShardedReadOnlyIndexTable<u32, Address, Xxh3Partitioner> {
        ShardedReadOnlyIndexTable::new(
            Xxh3Partitioner::new(n),
            weak_dbs.clone(),
            pk_by_index_def,
            index_by_pk_def,
        ).expect("reader")
    }

    pub(crate) fn mk_sharded_writer(name: &str, n: usize, lru_cache: usize, weak_dbs: Vec<Weak<Database>>) -> (ShardedTableWriter<u32, Address, IndexFactory<u32, Address>, BytesPartitioner, Xxh3Partitioner>, MultimapTableDefinition<'static, Address, u32>, TableDefinition<'static, u32, Address>) {
        let pk_by_index_def = MultimapTableDefinition::<Address, u32>::new("pk_by_index");
        let index_by_pk_def = TableDefinition::<u32, Address>::new("index_by_pk");

        let writer = ShardedTableWriter::new(
            Partitioning::by_value(n),
            weak_dbs.clone(),
            IndexFactory::new(name, lru_cache, pk_by_index_def, index_by_pk_def, ),
        ).expect("writer");

        (writer, pk_by_index_def, index_by_pk_def)
    }

    pub(crate) fn setup_index_defs(lru_cap: usize) -> (
        Arc<Database>,
        Weak<Database>,
        LruCache<Vec<u8>, <u32 as CopyOwnedValue>::Unit>,
        MultimapTableDefinition<'static, Address, u32>, // pk_by_index
        TableDefinition<'static, u32, Address>,         // index_by_pk
    ) {
        let (owner_db, weak_db)   = test_utils::mk_db("redbit_index_test");
        let lru  = LruCache::new(NonZeroUsize::new(lru_cap).unwrap());

        let pk_by_index   = MultimapTableDefinition::<Address, u32>::new("pk_by_index");
        let index_by_pk   = TableDefinition::<u32, Address>::new("index_by_pk");
        (owner_db, weak_db, lru, pk_by_index, index_by_pk)
    }

    pub(crate) fn mk_index<'txn, 'c>(
        tx: &'txn WriteTransaction,
        cache: &'c mut LruCache<Vec<u8>, <u32 as CopyOwnedValue>::Unit>,
        pk_by_index_def: MultimapTableDefinition<'static, Address, u32>,
        index_by_pk_def: TableDefinition<'static, u32, Address>,
    ) -> IndexTable<'txn, 'c, u32, Address> {
        // Construct directly; mirrors your struct shape.
        IndexTable {
            pk_by_index: tx.open_multimap_table(pk_by_index_def).expect("open pk_by_index"),
            index_by_pk: tx.open_table(index_by_pk_def).expect("open index_by_pk"),
            cache: Some(cache),
        }
    }

}

#[cfg(all(test, not(feature = "integration")))]
pub mod dict_test_utils {
    use crate::storage::test_utils::{addr, Address};
    use crate::*;
    use lru::LruCache;
    use std::num::NonZeroUsize;

    pub (crate) fn mk_sharder_reader(n: usize, weak_dbs: Vec<Weak<Database>>, dict_pk_to_ids: MultimapTableDefinition<'static, u32, u32>, value_by_dict_pk: TableDefinition<'static, u32, Address>, value_to_dict_pk: TableDefinition<'static, Address, u32>, dict_pk_by_id: TableDefinition<'static, u32, u32>) -> ShardedReadOnlyDictTable<u32, Address, Xxh3Partitioner> {
        ShardedReadOnlyDictTable::new(
            Xxh3Partitioner::new(n),
            weak_dbs.clone(),
            dict_pk_to_ids,
            value_by_dict_pk,
            value_to_dict_pk,
            dict_pk_by_id,
        ).expect("reader")
    }

    pub(crate) fn mk_sharded_writer(name: &str, n: usize, weak_dbs: Vec<Weak<Database>>) -> (ShardedTableWriter<u32, Address, DictFactory<u32, Address>, BytesPartitioner, Xxh3Partitioner>, MultimapTableDefinition<'static, u32, u32>, TableDefinition<'static, u32, Address>, TableDefinition<'static, Address, u32>, TableDefinition<'static, u32, u32>) {
        // Table defs
        let dict_pk_to_ids   = MultimapTableDefinition::<u32, u32>::new("dict_pk_to_ids");
        let value_by_dict_pk = TableDefinition::<u32, Address>::new("value_by_dict_pk");
        let value_to_dict_pk = TableDefinition::<Address, u32>::new("value_to_dict_pk");
        let dict_pk_by_id    = TableDefinition::<u32, u32>::new("dict_pk_by_id");

        // Writer by value (so identical values go to the same shard):
        let writer = ShardedTableWriter::new(
            Partitioning::by_value(n),
            weak_dbs.clone(),
            DictFactory::new(
                name,
                8192, // LRU cap
                dict_pk_to_ids,
                value_by_dict_pk,
                value_to_dict_pk,
                dict_pk_by_id,
            ),
        ).expect("writer");

        (writer, dict_pk_to_ids, value_by_dict_pk, value_to_dict_pk, dict_pk_by_id)
    }

    pub(crate) fn setup_dict_defs(cap: usize) -> (
        Database,
        WriteTransaction,
        LruCache<Vec<u8>, <u32 as CopyOwnedValue>::Unit>,
        MultimapTableDefinition<'static, u32, u32>,
        TableDefinition<'static, u32, Address>,
        TableDefinition<'static, Address, u32>,
        TableDefinition<'static, u32, u32>,
    ) {
        let random_db_path = std::env::temp_dir().join(format!("redbit_test_{}", rand::random::<u64>()));
        let random_db = Database::builder().create(random_db_path).expect("Failed to create test db");
        let write_tx = random_db.begin_write().expect("Failed to begin write tx");
        let lru_cache = LruCache::new(NonZeroUsize::new(cap).unwrap());

        let dict_pk_to_ids: MultimapTableDefinition<'static, u32, u32> = MultimapTableDefinition::new("dict_pk_to_ids");
        let value_by_dict_pk: TableDefinition<'static, u32, Address> = TableDefinition::new("value_by_dict_pk");
        let value_to_dict_pk: TableDefinition<'static, Address, u32> = TableDefinition::new("value_to_dict_pk");
        let dict_pk_by_id: TableDefinition<'static, u32, u32> = TableDefinition::new("dict_pk_by_id");

        (random_db, write_tx, lru_cache, dict_pk_to_ids, value_by_dict_pk, value_to_dict_pk, dict_pk_by_id)
    }

    pub(crate) fn mk_dict<'txn, 'c>(
        tx: &'txn WriteTransaction,
        cache: &'c mut LruCache<Vec<u8>, <u32 as CopyOwnedValue>::Unit>,
        dict_pk_to_ids: MultimapTableDefinition<'static, u32, u32>,
        value_by_dict_pk: TableDefinition<'static, u32, Address>,
        value_to_dict_pk: TableDefinition<'static, Address, u32>,
        dict_pk_by_id: TableDefinition<'static, u32, u32>,
    ) -> DictTable<'txn, 'c, u32, Address> {
        DictTable::new(tx, cache, dict_pk_to_ids, value_by_dict_pk, value_to_dict_pk, dict_pk_by_id).expect("Failed to create DictTable")
    }

    /// Insert a pair (id, value bytes).
    pub(crate) fn dict_insert(dict: &mut DictTable<'_, '_, u32, Address>, id: u32, v: &[u8]) {
        dict.insert_kv(&id, &addr(v)).expect("insert_kv");
    }

    /// Read the birth id for a given external id.
    pub(crate) fn birth_id_of(dict: &DictTable<'_, '_, u32, Address>, id: u32) -> u32 {
        dict.dict_pk_by_id.get(&id).expect("get").expect("missing").value()
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
}