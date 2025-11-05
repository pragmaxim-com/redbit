use crate::storage::table_plain::PlainFactory;
use crate::storage::table_writer_api::{ReadTableFactory, ReadTableLike, ShardedTableReader, TableFactory};
use crate::{AppError, CopyOwnedValue, KeyPartitioner, Partitioning, TableInfo, ValuePartitioner};
use redb::{AccessGuard, Database, Key, MultimapRange, MultimapValue, ReadOnlyTable, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};
use std::borrow::Borrow;
use std::ops::RangeBounds;
use std::sync::Weak;

pub struct ReadOnlyPlainTable<K: Key + 'static, V: Key + 'static> {
    underlying: ReadOnlyTable<K, V>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyPlainTable<K, V> {
    pub fn new(db_weak: &Weak<Database>, underlying_def: TableDefinition<K, V>) -> Result<Self, AppError> {
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
    shards: Vec<ReadOnlyPlainTable<K, V>>,
    pk_partitioner: PK,
}

impl<K, V, PK> ShardedReadOnlyPlainTable<K, V, PK>
where
    K: CopyOwnedValue + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    PK: KeyPartitioner<K>,
{
    /// Build a sharded reader. Requires at least 2 DBs.
    pub fn new(pk_partitioner: PK, dbs: Vec<Weak<Database>>, factory: &PlainFactory<K, V>) -> Result<Self, AppError> {
        let mut shards = Vec::with_capacity(dbs.len());
        for db_weak in &dbs {
            shards.push(factory.open_for_read(db_weak)?);
        }
        Ok(Self { shards, pk_partitioner })
    }
}

impl<K, V, KP, VP> ReadTableFactory<K, V, KP, VP> for PlainFactory<K, V>
where
    K: CopyOwnedValue + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    KP: KeyPartitioner<K> + Sync + Send + Clone + 'static,
    VP: ValuePartitioner<V> + Sync + Send + Clone + 'static,
{
    fn build_sharded_reader(&self, dbs: Vec<Weak<Database>>, partitioning: &Partitioning<KP, VP>) -> Result<ShardedTableReader<K, V, KP, VP>, AppError> {
        match partitioning {
            Partitioning::ByKey(kp) => {
                let table = ShardedReadOnlyPlainTable::new(kp.clone(), dbs, self)?;
                Ok(ShardedTableReader::Plain(table))
            }
            Partitioning::ByValue(_) => {
                Err(AppError::Custom("PlainFactory does not support value partitioning".to_string()))
            }
        }
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
