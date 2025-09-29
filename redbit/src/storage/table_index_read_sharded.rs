use crate::{AppError, KeyPartitioner, TableInfo};
use redb::{AccessGuard, Database, Key, MultimapTableDefinition, MultimapValue, ReadOnlyMultimapTable, ReadOnlyTable, ReadableDatabase, ReadableTableMetadata, StorageError, TableDefinition};
use std::borrow::Borrow;
use std::sync::Weak;

struct ReadOnlyIndexTableShard<K: Key + 'static, V: Key + 'static> {
    pk_by_index: ReadOnlyMultimapTable<V, K>,
    index_by_pk: ReadOnlyTable<K, V>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyIndexTableShard<K, V> {
    fn open(db_weak: &Weak<Database>, pk_by_index_def: MultimapTableDefinition<V, K>, index_by_pk_def: TableDefinition<K, V>) -> Result<Self, AppError> {
        let db_arc = db_weak.upgrade().ok_or_else(|| AppError::Custom("database closed".to_string()))?;
        let tx = db_arc.begin_read()?;
        Ok(Self {
            pk_by_index: tx.open_multimap_table(pk_by_index_def)?,
            index_by_pk: tx.open_table(index_by_pk_def)?,
        })
    }
}

pub struct ShardedReadOnlyIndexTable<K, V, PK>
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    PK: KeyPartitioner<K>,
{
    shards: Vec<ReadOnlyIndexTableShard<K, V>>,
    pk_partitioner: PK, // route by primary key
}

impl<K, V, PK> ShardedReadOnlyIndexTable<K, V, PK>
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    PK: KeyPartitioner<K>,
{
    pub fn new(pk_partitioner: PK,
        dbs: Vec<Weak<Database>>,
        pk_by_index_def: MultimapTableDefinition<V, K>,
        index_by_pk_def: TableDefinition<K, V>,
    ) -> Result<Self, AppError> {
        if dbs.len() < 2 {
            return Err(AppError::Custom(format!(
                "ShardedReadOnlyIndexTable expected at least 2 databases, got {}",
                dbs.len()
            )));
        }
        let mut shards = Vec::with_capacity(dbs.len());
        for db_weak in &dbs {
            shards.push(ReadOnlyIndexTableShard::open(
                db_weak,
                pk_by_index_def,
                index_by_pk_def,
            )?);
        }
        Ok(Self {
            shards,
            pk_partitioner,
        })
    }

    fn shard_by_k<'k>(&self, key: impl Borrow<K::SelfType<'k>>) -> usize {
        let sid = self.pk_partitioner.partition_key(key.borrow());
        debug_assert!(sid < self.shards.len());
        sid
    }

    pub fn get_value<'k>(&self, key: impl Borrow<K::SelfType<'k>>) -> redb::Result<Option<AccessGuard<'_, V>>> {
        let sid = self.shard_by_k(key.borrow());
        let shard = &self.shards[sid];
        shard.index_by_pk.get(key.borrow())
    }

    pub fn get_keys<'v>(&self, val: impl Borrow<V::SelfType<'v>>) -> redb::Result<MultimapValue<'static, K>> {
        for shard in &self.shards {
            let mmv = shard.pk_by_index.get(val.borrow())?;
            if !mmv.is_empty() {
                return Ok(mmv);
            }
        }
        Err(StorageError::Corrupted("ShardedReadOnlyIndexTable::get_keys found no entries".to_string()))
    }

    /// Aggregated stats: sum len() across shards for pk_by_index,
    /// use the first shard's pk_by_index.stats() as representative.
    pub fn stats(&self) -> redb::Result<Vec<TableInfo>> {
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

    /// Range is intentionally not implemented in the sharded variant.
    pub fn range_keys<'a, KR: Borrow<V::SelfType<'a>>>(&self,
        _range: impl std::ops::RangeBounds<KR>,
    ) -> redb::Result<redb::MultimapRange<'static, V, K>> {
        unimplemented!()
    }
}
