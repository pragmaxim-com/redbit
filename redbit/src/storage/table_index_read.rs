use crate::storage::table_index::IndexFactory;
use crate::storage::table_writer_api::{ReadTableFactory, ReadTableLike, ShardedTableReader, TableFactory};
use crate::{AppError, CacheKey, CopyOwnedValue, KeyPartitioner, Partitioning, TableInfo, ValuePartitioner};
use redb::{AccessGuard, Database, Key, MultimapTableDefinition, MultimapValue, Range, ReadOnlyMultimapTable, ReadOnlyTable, ReadableDatabase, ReadableTableMetadata, TableDefinition};
use std::borrow::Borrow;
use std::ops::RangeBounds;
use std::sync::Weak;

pub struct ReadOnlyIndexTable<K: Key + 'static, V: Key + 'static> {
    pk_by_index: ReadOnlyMultimapTable<V, K>,
    index_by_pk: ReadOnlyTable<K, V>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyIndexTable<K, V> {
    pub fn new(db_weak: &Weak<Database>, pk_by_index_def: MultimapTableDefinition<V, K>, index_by_pk_def: TableDefinition<K, V>) -> Result<Self, AppError> {
        let db_arc = db_weak.upgrade().ok_or_else(|| AppError::Custom("database closed".to_string()))?;
        let tx = db_arc.begin_read()?;
        Ok(Self {
            pk_by_index: tx.open_multimap_table(pk_by_index_def)?,
            index_by_pk: tx.open_table(index_by_pk_def)?,
        })
    }
}

pub struct ShardedReadOnlyIndexTable<K, V, VP>
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    VP: ValuePartitioner<V>,
{
    shards: Vec<ReadOnlyIndexTable<K, V>>,
    value_partitioner: VP,
}

impl<K, V, VP> ShardedReadOnlyIndexTable<K, V, VP>
where
    K: CopyOwnedValue + 'static + Borrow<K::SelfType<'static>>,
    V: CacheKey + 'static + Borrow<V::SelfType<'static>>,
    VP: ValuePartitioner<V>,
{
    pub fn new(value_partitioner: VP, dbs: Vec<Weak<Database>>, factory: &IndexFactory<K, V>) -> Result<Self, AppError> {
        let mut shards = Vec::with_capacity(dbs.len());
        for db_weak in &dbs {
            shards.push(factory.open_for_read(db_weak)?);
        }
        Ok(Self { shards, value_partitioner })
    }
}

impl<K, V, KP, VP> ReadTableFactory<K, V, KP, VP> for IndexFactory<K, V>
where
    K: CopyOwnedValue + 'static + Borrow<K::SelfType<'static>>,
    V: CacheKey + 'static + Borrow<V::SelfType<'static>>,
    KP: KeyPartitioner<K> + Sync + Send + Clone + 'static,
    VP: ValuePartitioner<V> + Sync + Send + Clone + 'static,
{
    fn build_sharded_reader(&self, dbs: Vec<Weak<Database>>, partitioning: &Partitioning<KP, VP>) -> Result<ShardedTableReader<K, V, KP, VP>, AppError> {
        match partitioning {
            Partitioning::ByKey(_) => {
                Err(AppError::Custom("IndexFactory does not support key partitioning".to_string()))
            }
            Partitioning::ByValue(vp) => {
                let table = ShardedReadOnlyIndexTable::<K, V, VP>::new(vp.clone(), dbs, self)?;
                Ok(ShardedTableReader::Index(table))
            }
        }
    }
}

impl<K, V, VP> ReadTableLike<K, V> for ShardedReadOnlyIndexTable<K, V, VP>
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    VP: ValuePartitioner<V>,
{

    fn get_value<'k>(&self, key: impl Borrow<K::SelfType<'k>>) -> Result<Option<AccessGuard<'_, V>>, AppError> {
        for shard in &self.shards {
            if let Some(vg) = shard.index_by_pk.get(key.borrow())? {
                return Ok(Some(vg));
            }
        }
        Ok(None)
    }

    fn index_keys<'v>(&self, val: impl Borrow<V::SelfType<'v>>) -> Result<MultimapValue<'static, K>, AppError> {
        let shard =
            if self.shards.len() == 1 {
                &self.shards[0]
            } else {
                &self.shards[self.value_partitioner.partition_value(val.borrow())]
            };
        Ok(shard.pk_by_index.get(val.borrow())?)
    }

    fn index_range<'a, KR: Borrow<V::SelfType<'a>>>(&self, range: impl RangeBounds<KR>) -> Result<redb::MultimapRange<'static, V, K>, AppError> {
        let shard =
            if self.shards.len() == 1 {
                &self.shards[0]
            } else {
                unimplemented!()
            };
        Ok(shard.pk_by_index.range(range)?)
    }

    /// Aggregated stats: sum len() across shards for pk_by_index, use the first shard's pk_by_index.stats() as representative.
    fn stats(&self) -> Result<Vec<TableInfo>, AppError> {
        debug_assert!(!self.shards.is_empty());
        let mut total: u64 = 0;
        for s in &self.shards {
            total = total.saturating_add(s.pk_by_index.len()?);
        }
        let rep_stats = self.shards[0].pk_by_index.stats()?;
        Ok(vec![TableInfo::from_stats(
            "pk_by_index",
            total,
            rep_stats,
        )])
    }

    fn iter_keys(&self) -> Result<Range<'_, K, V>, AppError> {
        unimplemented!()
    }

    fn range<'a, KR: Borrow<K::SelfType<'a>>>(&self, _range: impl RangeBounds<KR>) -> Result<Range<'static, K, V>, AppError> {
        unimplemented!()
    }

    fn last_key(&self) -> Result<Option<(AccessGuard<'_, K>, AccessGuard<'_, V>)>, AppError> {
        unimplemented!()
    }

    fn first_key(&self) -> Result<Option<(AccessGuard<'_, K>, AccessGuard<'_, V>)>, AppError> {
        unimplemented!()
    }

    fn dict_keys<'v>(&self, _val: impl Borrow<V::SelfType<'v>>) -> redb::Result<Option<MultimapValue<'static, K>>, AppError> {
        unimplemented!()
    }
}
