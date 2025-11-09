use crate::storage::partitioning::ValuePartitioner;
use crate::storage::table_dict::DictFactory;
use crate::storage::table_writer_api::{ReadTableFactory, ShardedTableReader, TableFactory, TableInfo};
use crate::{AppError, CacheKey, DbKey, KeyPartitioner, Partitioning, ReadTableLike, DbVal};
use redb::Key;
use redb::*;
use std::borrow::Borrow;
use std::ops::RangeBounds;
use std::sync::Weak;

pub struct ReadOnlyDictTable<K: Key + 'static, V: Key + 'static> {
    dict_pk_to_ids: ReadOnlyMultimapTable<K, K>,
    value_by_dict_pk: ReadOnlyTable<K, V>,
    value_to_dict_pk: ReadOnlyTable<V, K>,
    dict_pk_by_id: ReadOnlyTable<K, K>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyDictTable<K, V> {
    pub fn new(
        db_weak: &Weak<Database>,
        dict_pk_to_ids_def: MultimapTableDefinition<K, K>,
        value_by_dict_pk_def: TableDefinition<K, V>,
        value_to_dict_pk_def: TableDefinition<V, K>,
        dict_pk_by_id_def: TableDefinition<K, K>,
    ) -> Result<Self, AppError> {
        let db_arc = db_weak.upgrade().ok_or_else(|| AppError::Custom("database closed".to_string()))?;
        let tx = db_arc.begin_read()?;
        Ok(Self {
            dict_pk_to_ids: tx.open_multimap_table(dict_pk_to_ids_def)?,
            value_by_dict_pk: tx.open_table(value_by_dict_pk_def)?,
            value_to_dict_pk: tx.open_table(value_to_dict_pk_def)?,
            dict_pk_by_id: tx.open_table(dict_pk_by_id_def)?,
        })
    }
}

pub struct ShardedReadOnlyDictTable<K: DbKey, V: DbVal, VP: ValuePartitioner<V>> {
    shards: Vec<ReadOnlyDictTable<K, V>>,
    value_partitioner: VP,
}

impl<K: DbKey, V: CacheKey, VP: ValuePartitioner<V>> ShardedReadOnlyDictTable<K, V, VP> {
    pub fn new(value_partitioner: VP, dbs: Vec<Weak<Database>>, factory: &DictFactory<K, V>) -> Result<Self, AppError> {
        let mut shards = Vec::with_capacity(dbs.len());
        for db_weak in &dbs {
            shards.push(factory.open_for_read(db_weak)?);
        }
        Ok(Self { shards, value_partitioner })
    }
}

impl<K: DbKey, V: CacheKey, KP: KeyPartitioner<K>, VP: ValuePartitioner<V>> ReadTableFactory<K, V, KP, VP> for DictFactory<K, V> {
    fn build_sharded_reader(&self, dbs: Vec<Weak<Database>>, partitioning: &Partitioning<KP, VP>) -> std::result::Result<ShardedTableReader<K, V, KP, VP>, AppError> {
        match partitioning {
            Partitioning::ByKey(_) => {
                Err(AppError::Custom("DictFactory does not support key partitioning".to_string()))
            }
            Partitioning::ByValue(vp) => {
                Ok(ShardedTableReader::Dict(ShardedReadOnlyDictTable::new(vp.clone(), dbs, self)?))
            }
        }
    }
}


impl<K: DbKey, V: DbVal, VP: ValuePartitioner<V>> ReadTableLike<K, V> for ShardedReadOnlyDictTable<K, V, VP> {

    fn get_value<'k>(&self, key: impl Borrow<K::SelfType<'k>>) -> Result<Option<AccessGuard<'_, V>>, AppError> {
        for shard in &self.shards {
            if let Some(birth_guard) = shard.dict_pk_by_id.get(key.borrow())? {
                let birth_id = birth_guard.value();
                let val_guard = shard.value_by_dict_pk.get(birth_id)?;
                return Ok(val_guard);
            }
        }
        Ok(None)
    }

    fn dict_keys<'v>(&self, val: impl Borrow<V::SelfType<'v>>) -> Result<Option<MultimapValue<'static, K>>, AppError> {
        let shard = if self.shards.len() == 1 {
            &self.shards[0]
        } else {
            &self.shards[self.value_partitioner.partition_value(val.borrow())]
        };
        let birth_guard = shard.value_to_dict_pk.get(val.borrow())?;
        match birth_guard {
            Some(g) => {
                let birth_id = g.value();
                let value = shard.dict_pk_to_ids.get(&birth_id)?;
                Ok(Some(value))
            },
            None => Ok(None),
        }
    }

    fn stats(&self) -> Result<Vec<TableInfo>, AppError> {
        debug_assert!(!self.shards.is_empty());
        let mut total: u64 = 0;
        for s in &self.shards {
            total = total.saturating_add(s.value_by_dict_pk.len()?);
        }
        let rep_stats = self.shards[0].value_by_dict_pk.stats()?;
        Ok(vec![TableInfo::from_stats("distinct_values", total, rep_stats)])
    }

    fn index_keys<'v>(&self, _val: impl Borrow<V::SelfType<'v>>) -> Result<MultimapValue<'static, K>, AppError> {
        unimplemented!()
    }

    fn iter_keys(&self) -> Result<Range<'_, K, V>, AppError> {
        unimplemented!()
    }

    fn range<'a, KR: Borrow<K::SelfType<'a>>>(&self, _range: impl RangeBounds<KR>) -> Result<Range<'static, K, V>, AppError> {
        unimplemented!()
    }

    fn index_range<'a, KR: Borrow<V::SelfType<'a>>>(&self, _range: impl RangeBounds<KR>) -> Result<MultimapRange<'static, V, K>, AppError> {
        unimplemented!()
    }

    fn last_key(&self) -> Result<Option<(AccessGuard<'_, K>, AccessGuard<'_, V>)>, AppError> {
        unimplemented!()
    }

    fn first_key(&self) -> Result<Option<(AccessGuard<'_, K>, AccessGuard<'_, V>)>, AppError> {
        unimplemented!()
    }
}
