use crate::storage::partitioning::ValuePartitioner;
use crate::{AppError, TableInfo};
use redb::Key;
use redb::*;
use std::borrow::Borrow;
use std::sync::Weak;

struct ReadOnlyDictTableShard<K: Key + 'static, V: Key + 'static> {
    dict_pk_to_ids: ReadOnlyMultimapTable<K, K>,
    value_by_dict_pk: ReadOnlyTable<K, V>,
    value_to_dict_pk: ReadOnlyTable<V, K>,
    dict_pk_by_id: ReadOnlyTable<K, K>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyDictTableShard<K, V> {
    fn open(
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

pub struct ShardedReadOnlyDictTable<K, V, VP>
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    VP: ValuePartitioner<V>,
{
    shards: Vec<ReadOnlyDictTableShard<K, V>>,
    value_partitioner: VP,
}

impl<K, V, VP> ShardedReadOnlyDictTable<K, V, VP>
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    VP: ValuePartitioner<V>
{
    /// Build a sharded reader. `dbs.len()` must equal `layout.get()`.
    pub fn new(
        val_partitioner: VP,
        dbs: Vec<Weak<Database>>,
        dict_pk_to_ids_def: MultimapTableDefinition<K, K>,
        value_by_dict_pk_def: TableDefinition<K, V>,
        value_to_dict_pk_def: TableDefinition<V, K>,
        dict_pk_by_id_def: TableDefinition<K, K>,
    ) -> Result<Self, AppError> {
        if dbs.len() < 2 {
            return Err(AppError::Custom(format!("ShardedReadOnlyDictTable expected at least 2 databases, got {}", dbs.len())));
        }
        let mut shards = Vec::with_capacity(dbs.len());
        for db_weak in &dbs {
            shards.push(ReadOnlyDictTableShard::open(
                db_weak,
                dict_pk_to_ids_def,
                value_by_dict_pk_def,
                value_to_dict_pk_def,
                dict_pk_by_id_def,
            )?);
        }
        Ok(Self { shards, value_partitioner: val_partitioner })
    }

    pub fn get_value<'k>(&self, key: impl Borrow<K::SelfType<'k>>) -> Result<Option<AccessGuard<'_, V>>> {
        for shard in &self.shards {
            if let Some(birth_guard) = shard.dict_pk_by_id.get(key.borrow())? {
                let birth_id = birth_guard.value();
                let val_guard = shard.value_by_dict_pk.get(birth_id)?;
                return Ok(val_guard);
            }
        }
        Ok(None)
    }

    /// Complexity: O(#shards) worst-case for the initial search; then O(1).
    pub fn get_keys<'v>(&self, val: impl Borrow<V::SelfType<'v>>) -> Result<Option<MultimapValue<'static, K>>, AppError> {
        let sid = self.value_partitioner.partition_value(val.borrow());
        let shard = &self.shards[sid];
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

    /// Sums len() over all shards and uses the first shard's stats() as a representative.
    pub fn stats(&self) -> Result<Vec<TableInfo>, AppError> {
        debug_assert!(!self.shards.is_empty());
        let mut total: u64 = 0;
        for s in &self.shards {
            total = total.saturating_add(s.value_by_dict_pk.len()?);
        }
        let rep_stats = self.shards[0].value_by_dict_pk.stats()?;
        Ok(vec![TableInfo::from_stats("distinct_values", total, rep_stats)])
    }
}
