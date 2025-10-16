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
pub mod async_boundary;
pub mod table_writer_api;
mod cache_bench;
mod router;
mod sort_buffer;

#[cfg(all(test, not(feature = "integration")))]
pub mod test_utils {
    use crate::CacheKey;
    use redb::{Database, Key, TypeName, Value};
    use std::cmp::Ordering;
    use std::sync::{Arc, Weak};

    #[derive(Debug, Clone, Copy)]
    pub struct TxHash(pub [u8; 32]);
    impl Value for TxHash {
        type SelfType<'a> = TxHash where Self: 'a;
        type AsBytes<'a> = &'a [u8; 32] where Self: 'a;
        fn fixed_width() -> Option<usize> {
            Some(32)
        }
        fn from_bytes<'a>(data: &'a [u8]) -> TxHash
        where
            Self: 'a,
        {
            TxHash(data.try_into().unwrap())
        }
        fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> &'a [u8; 32]
        where
            Self: 'a,
            Self: 'b,
        {
            &value.0
        }
        fn type_name() -> TypeName {
            TypeName::new("[u8;32]")
        }
    }
    impl Key for TxHash {
        fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
            data1.cmp(data2)
        }
    }
    impl CacheKey for TxHash {
        type CK = [u8; 32];
        #[inline]
        fn cache_key<'a>(v: &<TxHash as Value>::SelfType<'a>) -> Self::CK
        where
            TxHash: 'a,
        {
            v.0
        }
    }

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

    pub(crate) fn txh(prefix: &[u8]) -> TxHash {
        let mut b = [0u8; 32];
        let n = core::cmp::min(prefix.len(), 32);
        b[..n].copy_from_slice(&prefix[..n]);
        TxHash(b)
    }

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
    use crate::*;
    use redb::{Database, Durability, TableDefinition, WriteTransaction};
    use std::borrow::Borrow;

    pub(crate) fn mk_sharded_reader<V: CacheKey + Send + Clone + 'static + Borrow<V::SelfType<'static>>>(n: usize, weak_dbs: Vec<Weak<Database>>, plain_def: TableDefinition<'static, u32, V>) -> ShardedReadOnlyPlainTable<u32, V, BytesPartitioner> {
        ShardedReadOnlyPlainTable::new(
            BytesPartitioner::new(n),
            weak_dbs.clone(),
            plain_def,
        ).expect("reader")
    }

    pub(crate) fn mk_sharded_writer<V: CacheKey + Send + Clone + 'static + Borrow<V::SelfType<'static>>>(name: &str, n: usize, weak_dbs: Vec<Weak<Database>>) -> (ShardedTableWriter<u32, V, PlainFactory<u32, V>, BytesPartitioner, Xxh3Partitioner>, TableDefinition<'static, u32, V>) {
        let plain_def = TableDefinition::<u32, V>::new("plain_underlying");

        // Writer/Reader
        let writer = ShardedTableWriter::new(
            Partitioning::by_key(n),
            weak_dbs.clone(),
            PlainFactory::new(name, plain_def),
            Durability::None
        ).expect("writer");

        (writer, plain_def)
    }

    pub(crate) fn setup_plain_defs<V: CacheKey + Send + Clone + 'static + Borrow<V::SelfType<'static>>>() -> (
        Database,
        WriteTransaction,
        TableDefinition<'static, u32, V>,
    ) {
        let path = std::env::temp_dir().join(format!("redbit_plain_test_{}", rand::random::<u64>()));
        let db   = Database::builder().create(path).expect("create db");
        let tx   = db.begin_write().expect("begin write");
        let tbl  = TableDefinition::<u32, V>::new("plain_underlying");
        (db, tx, tbl)
    }

    pub(crate) fn mk_plain<'txn, V: CacheKey>(tx: &'txn WriteTransaction, underlying_def: TableDefinition<'static, u32, V>) -> PlainTable<'txn, u32, V> {
        PlainTable {
            table: tx.open_table(underlying_def).expect("open plain table"),
        }
    }
}

#[cfg(all(test, not(feature = "integration")))]
pub mod index_test_utils {
    use crate::storage::test_utils;
    use crate::*;
    use lru::LruCache;
    use redb::{Database, Durability, MultimapTableDefinition, TableDefinition, WriteTransaction};
    use std::borrow::Borrow;
    use std::num::NonZeroUsize;

    pub(crate) fn mk_sharded_reader<V: CacheKey + Send + Clone + 'static + Borrow<V::SelfType<'static>>>(n: usize, weak_dbs: Vec<Weak<Database>>, pk_by_index_def: MultimapTableDefinition<'static, V, u32>, index_by_pk_def: TableDefinition<'static, u32, V>) -> ShardedReadOnlyIndexTable<u32, V, Xxh3Partitioner> {
        ShardedReadOnlyIndexTable::new(
            Xxh3Partitioner::new(n),
            weak_dbs.clone(),
            pk_by_index_def,
            index_by_pk_def,
        ).expect("reader")
    }

    pub(crate) fn mk_sharded_writer<V: CacheKey + Send + Clone + 'static + Borrow<V::SelfType<'static>>>(name: &str, n: usize, lru_cache: usize, weak_dbs: Vec<Weak<Database>>) -> (ShardedTableWriter<u32, V, IndexFactory<u32, V>, BytesPartitioner, Xxh3Partitioner>, MultimapTableDefinition<'static, V, u32>, TableDefinition<'static, u32, V>) {
        let pk_by_index_def = MultimapTableDefinition::<V, u32>::new("pk_by_index");
        let index_by_pk_def = TableDefinition::<u32, V>::new("index_by_pk");

        let writer = ShardedTableWriter::new(
            Partitioning::by_value(n),
            weak_dbs.clone(),
            IndexFactory::new(name, lru_cache, pk_by_index_def, index_by_pk_def),
            Durability::None
        ).expect("writer");

        (writer, pk_by_index_def, index_by_pk_def)
    }

    pub(crate) fn setup_index_defs<K: CopyOwnedValue + Send + Clone + 'static + Borrow<K::SelfType<'static>>, V: CacheKey + Send + Clone + 'static + Borrow<V::SelfType<'static>>>
        (name: &str, lru_cap: usize) -> (
        Arc<Database>,
        TableWriter<K, V, IndexFactory<K, V>>,
        LruCache<V::CK, K::Unit>,
        MultimapTableDefinition<'static, V, K>, // pk_by_index
        TableDefinition<'static, K, V>,         // index_by_pk
    ) {
        let (owner_db, weak_db)   = test_utils::mk_db("redbit_index_test");
        let lru  = LruCache::new(NonZeroUsize::new(lru_cap).unwrap());

        let pk_by_index   = MultimapTableDefinition::<V, K>::new("pk_by_index");
        let index_by_pk   = TableDefinition::<K, V>::new("index_by_pk");
        let writer = TableWriter::new(weak_db, IndexFactory::new(name, lru_cap, pk_by_index, index_by_pk), Durability::None).expect("new writer");
        (owner_db, writer, lru, pk_by_index, index_by_pk)
    }

    pub(crate) fn mk_index<'txn, 'c, K: CopyOwnedValue, V: CacheKey>(
        tx: &'txn WriteTransaction,
        cache: &'c mut LruCache<V::CK, K::Unit>,
        pk_by_index_def: MultimapTableDefinition<'static, V, K>,
        index_by_pk_def: TableDefinition<'static, K, V>,
    ) -> IndexTable<'txn, 'c, K, V> {
        IndexTable {
            pk_by_index: tx.open_multimap_table(pk_by_index_def).expect("open pk_by_index"),
            index_by_pk: tx.open_table(index_by_pk_def).expect("open index_by_pk"),
            cache: Some(cache),
        }
    }

}

#[cfg(all(test, not(feature = "integration")))]
pub mod dict_test_utils {
    use crate::*;
    use lru::LruCache;
    use std::borrow::Borrow;
    use std::num::NonZeroUsize;
    use redb::Durability;

    pub (crate) fn mk_sharder_reader<V: CacheKey + Send + Clone + 'static + Borrow<V::SelfType<'static>>>(n: usize, weak_dbs: Vec<Weak<Database>>, dict_pk_to_ids: MultimapTableDefinition<'static, u32, u32>, value_by_dict_pk: TableDefinition<'static, u32, V>, value_to_dict_pk: TableDefinition<'static, V, u32>, dict_pk_by_id: TableDefinition<'static, u32, u32>) -> ShardedReadOnlyDictTable<u32, V, Xxh3Partitioner> {
        ShardedReadOnlyDictTable::new(
            Xxh3Partitioner::new(n),
            weak_dbs.clone(),
            dict_pk_to_ids,
            value_by_dict_pk,
            value_to_dict_pk,
            dict_pk_by_id,
        ).expect("reader")
    }

    pub(crate) fn mk_sharded_writer<V: CacheKey + Send + Clone + 'static + Borrow<V::SelfType<'static>>>(name: &str, n: usize, weak_dbs: Vec<Weak<Database>>) -> (ShardedTableWriter<u32, V, DictFactory<u32, V>, BytesPartitioner, Xxh3Partitioner>, MultimapTableDefinition<'static, u32, u32>, TableDefinition<'static, u32, V>, TableDefinition<'static, V, u32>, TableDefinition<'static, u32, u32>) {
        // Table defs
        let dict_pk_to_ids   = MultimapTableDefinition::<u32, u32>::new("dict_pk_to_ids");
        let value_by_dict_pk = TableDefinition::<u32, V>::new("value_by_dict_pk");
        let value_to_dict_pk = TableDefinition::<V, u32>::new("value_to_dict_pk");
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
            Durability::None
        ).expect("writer");

        (writer, dict_pk_to_ids, value_by_dict_pk, value_to_dict_pk, dict_pk_by_id)
    }

    pub(crate) fn setup_dict_defs<K: CopyOwnedValue + Send + Clone + 'static + Borrow<K::SelfType<'static>>, V: CacheKey + Send + Clone + 'static + Borrow<V::SelfType<'static>>>(cap: usize) -> (
        Database,
        WriteTransaction,
        LruCache<V::CK, K::Unit>,
        MultimapTableDefinition<'static, K, K>,
        TableDefinition<'static, K, V>,
        TableDefinition<'static, V, K>,
        TableDefinition<'static, K, K>,
    ) {
        let random_db_path = std::env::temp_dir().join(format!("redbit_test_{}", rand::random::<u64>()));
        let random_db = Database::builder().create(random_db_path).expect("Failed to create test db");
        let write_tx = random_db.begin_write().expect("Failed to begin write tx");
        let lru_cache = LruCache::new(NonZeroUsize::new(cap).unwrap());

        let dict_pk_to_ids: MultimapTableDefinition<'static, K, K> = MultimapTableDefinition::new("dict_pk_to_ids");
        let value_by_dict_pk: TableDefinition<'static, K, V> = TableDefinition::new("value_by_dict_pk");
        let value_to_dict_pk: TableDefinition<'static, V, K> = TableDefinition::new("value_to_dict_pk");
        let dict_pk_by_id: TableDefinition<'static, K, K> = TableDefinition::new("dict_pk_by_id");

        (random_db, write_tx, lru_cache, dict_pk_to_ids, value_by_dict_pk, value_to_dict_pk, dict_pk_by_id)
    }

    pub(crate) fn mk_dict<'txn, 'c, K: CopyOwnedValue, V: CacheKey>(
        tx: &'txn WriteTransaction,
        cache: &'c mut LruCache<V::CK, K::Unit>,
        dict_pk_to_ids: MultimapTableDefinition<'static, K, K>,
        value_by_dict_pk: TableDefinition<'static, K, V>,
        value_to_dict_pk: TableDefinition<'static, V, K>,
        dict_pk_by_id: TableDefinition<'static, K, K>,
    ) -> DictTable<'txn, 'c, K, V> {
        DictTable::new(tx, Some(cache), dict_pk_to_ids, value_by_dict_pk, value_to_dict_pk, dict_pk_by_id).expect("Failed to create DictTable")
    }
}