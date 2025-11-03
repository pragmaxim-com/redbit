use crate::storage::table_writer_api::ReadTableLike;
use crate::{AppError, KeyPartitioner, TableInfo};
use redb::{AccessGuard, Database, Key, MultimapRange, MultimapValue, ReadOnlyTable, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};
use std::borrow::Borrow;
use std::ops::RangeBounds;
use std::sync::Weak;

struct ReadOnlyPlainTableShard<K: Key + 'static, V: Key + 'static> {
    underlying: ReadOnlyTable<K, V>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyPlainTableShard<K, V> {
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
    pk_partitioner: PK,
}

impl<K, V, PK> ShardedReadOnlyPlainTable<K, V, PK>
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    PK: KeyPartitioner<K>,
{
    /// Build a sharded reader. Requires at least 2 DBs.
    pub fn new(pk_partitioner: PK, dbs: Vec<Weak<Database>>, underlying_def: TableDefinition<K, V>) -> Result<Self, AppError> {
        if dbs.len() < 1 {
            return Err(AppError::Custom(format!(
                "ShardedReadOnlyPlainTable expected at least 1 database, got {}",
                dbs.len()
            )));
        }
        let mut shards = Vec::with_capacity(dbs.len());
        for db_weak in &dbs {
            shards.push(ReadOnlyPlainTableShard::open(db_weak, underlying_def)?);
        }
        Ok(Self { shards, pk_partitioner })
    }
}

impl<K, V, PK> ReadTableLike<K, V> for ShardedReadOnlyPlainTable<K, V, PK>
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    PK: KeyPartitioner<K>,
{
    fn get_value<'k>(&self, key: impl Borrow<K::SelfType<'k>>) -> Result<Option<AccessGuard<'_, V>>, AppError> {
        let shard =
            if self.shards.len() == 1 {
                &self.shards[0]
            } else {
                &self.shards[self.pk_partitioner.partition_key(key.borrow())]
            };
        Ok(shard.underlying.get(key.borrow())?)
    }

    fn first_key(&self) -> Result<Option<(AccessGuard<'_, K>, AccessGuard<'_, V>)>, AppError> {
        let shard =
            if self.shards.len() == 1 {
                &self.shards[0]
            } else {
                unimplemented!()
            };
        Ok(shard.underlying.first()?)
    }

    fn last_key(&self) -> Result<Option<(AccessGuard<'_, K>, AccessGuard<'_, V>)>, AppError> {
        let shard =
            if self.shards.len() == 1 {
                &self.shards[0]
            } else {
                unimplemented!()
            };
        Ok(shard.underlying.last()?)
    }

    fn range<'a, KR: Borrow<K::SelfType<'a>>>(&self, range: impl RangeBounds<KR>) -> Result<redb::Range<'static, K, V>, AppError> {
        let shard =
            if self.shards.len() == 1 {
                &self.shards[0]
            } else {
                unimplemented!()
            };
        Ok(shard.underlying.range(range)?)
    }

    fn iter_keys(&self) -> Result<redb::Range<'_, K, V>, AppError> {
        let shard =
            if self.shards.len() == 1 {
                &self.shards[0]
            } else {
                unimplemented!()
            };
        Ok(shard.underlying.iter()?)
    }

    fn stats(&self) -> Result<Vec<TableInfo>, AppError> {
        let mut total: u64 = 0;
        for s in &self.shards {
            total = total.saturating_add(s.underlying.len()?);
        }
        let rep_stats = self.shards[0].underlying.stats()?;
        Ok(vec![TableInfo::from_stats("underlying", total, rep_stats)])
    }

    fn index_keys<'v>(&self, _val: impl Borrow<V::SelfType<'v>>) -> Result<MultimapValue<'static, K>, AppError> {
        unimplemented!()
    }

    fn index_range<'a, KR: Borrow<V::SelfType<'a>>>(&self, _range: impl RangeBounds<KR>) -> Result<MultimapRange<'static, V, K>, AppError> {
        unimplemented!()
    }

    fn dict_keys<'v>(&self, _val: impl Borrow<V::SelfType<'v>>) -> redb::Result<Option<MultimapValue<'static, K>>, AppError> {
        unimplemented!()
    }
}
