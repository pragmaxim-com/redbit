use crate::{AppError, KeyPartitioner, TableInfo};
use redb::{AccessGuard, Database, Key, ReadOnlyTable, ReadableDatabase, ReadableTableMetadata, TableDefinition};
use std::borrow::Borrow;
use std::sync::Weak;

struct ReadOnlyPlainTableShard<K: Key + 'static, V: Key + 'static> {
    underlying: ReadOnlyTable<K, V>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyPlainTableShard<K, V> {
    #[inline]
    fn open(db_weak: &Weak<Database>, underlying_def: TableDefinition<K, V>) -> Result<Self, AppError> {
        let db_arc = db_weak.upgrade().ok_or_else(|| AppError::Custom("database closed".to_string()))?;
        let tx = db_arc.begin_read()?;
        Ok(Self {
            underlying: tx.open_table(underlying_def)?,
        })
    }
}

pub struct ShardedReadOnlyPlainTable<K, V, PK>
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    PK: KeyPartitioner<K>,
{
    shards: Vec<ReadOnlyPlainTableShard<K, V>>,
    pk_partitioner: PK, // route by key
}

impl<K, V, PK> ShardedReadOnlyPlainTable<K, V, PK>
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    PK: KeyPartitioner<K>,
{
    /// Build a sharded reader. Requires at least 2 DBs.
    pub fn new(pk_partitioner: PK, dbs: Vec<Weak<Database>>, underlying_def: TableDefinition<K, V>) -> Result<Self, AppError> {
        if dbs.len() < 2 {
            return Err(AppError::Custom(format!(
                "ShardedReadOnlyPlainTable expected at least 2 databases, got {}",
                dbs.len()
            )));
        }
        let mut shards = Vec::with_capacity(dbs.len());
        for db_weak in &dbs {
            shards.push(ReadOnlyPlainTableShard::open(db_weak, underlying_def)?);
        }
        Ok(Self { shards, pk_partitioner })
    }

    fn shard_by_k<'k>(&self, key: impl Borrow<K::SelfType<'k>>) -> usize {
        let sid = self.pk_partitioner.partition_key(key.borrow());
        debug_assert!(sid < self.shards.len());
        sid
    }

    pub fn get_value<'k>(&self, key: impl Borrow<K::SelfType<'k>>) -> redb::Result<Option<AccessGuard<'_, V>>> {
        let sid = self.shard_by_k(key.borrow());
        let shard = &self.shards[sid];
        shard.underlying.get(key.borrow())
    }

    /// Aggregate stats: sum len() across shards, use shard 0's stats() as representative.
    pub fn stats(&self) -> redb::Result<Vec<TableInfo>> {
        debug_assert!(!self.shards.is_empty());
        let mut total: u64 = 0;
        for s in &self.shards {
            total = total.saturating_add(s.underlying.len()?);
        }
        let rep_stats = self.shards[0].underlying.stats()?;
        Ok(vec![TableInfo::from_stats("underlying", total, rep_stats)])
    }
}
