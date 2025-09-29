use crate::storage::partitioning::{KeyPartitioner, Partitioning, ValuePartitioner};
use crate::storage::table_writer::{TableFactory, ValueBuf, WriterCommand};
use crate::{AppError, FlushFuture, TableWriter};
use redb::{Database, Key};
use std::borrow::Borrow;
use std::{marker::PhantomData, sync::Weak};

pub struct ShardedTableWriter<
    K: Key + Send + Copy + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V> + Send + Clone + 'static,
    KP: KeyPartitioner<K> + Send + Sync + 'static,
    VP: ValuePartitioner<V> + Send + Sync + 'static,
> where F::Table<'static,'static>: Send
{
    partitioner: Partitioning<KP, VP>,
    shards: Vec<TableWriter<K, V, F>>,
    _pd: PhantomData<(K,V)>,
}

impl<
    K: Key + Send + Copy + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V> + Send + Clone + 'static,
    KP: KeyPartitioner<K> + Send + Sync + 'static,
    VP: ValuePartitioner<V> + Send + Sync + 'static,
> ShardedTableWriter<K,V,F, KP, VP>
where
    F::Table<'static,'static>: Send,
{
    pub fn new(partitioning: Partitioning<KP, VP>, dbs: Vec<Weak<Database>>, factory: F) -> redb::Result<Self, AppError> {
        if dbs.len() < 2 {
            return Err(AppError::Custom(format!("ShardedTableWriter: expected at least 2 databases, got {}", dbs.len())));
        }
        let mut shards = Vec::with_capacity(dbs.len());
        for db_weak in dbs.into_iter() {
            shards.push(TableWriter::<K,V,F>::new(db_weak, factory.clone())?);
        }
        Ok(Self { partitioner: partitioning, shards, _pd: PhantomData })
    }

    pub fn begin(&self) -> redb::Result<(), AppError> {
        for w in &self.shards { w.begin()?; }
        Ok(())
    }

    pub fn get_head_for_index(&self, values: Vec<V>) -> redb::Result<Vec<Option<ValueBuf<K>>>, AppError> {
        match &self.partitioner {
            Partitioning::ByKey(_) => {
                Err(AppError::Custom("ShardedTableWriter: get_head_for_index not supported with key partitioning".into()))
            }
            Partitioning::ByValue(vp) => {
                let shards_count = self.shards.len();
                let values_count = values.len();
                if values_count == 0 { return Ok(Vec::new()); }

                let mut buckets: Vec<Vec<(usize, V)>> = (0..shards_count).map(|_| Vec::new()).collect();
                for (pos, v) in values.into_iter().enumerate() {
                    let sid = vp.partition_value(v.borrow());
                    buckets[sid].push((pos, v));
                }

                let mut out: Vec<Option<ValueBuf<K>>> = Vec::with_capacity(values_count);
                out.resize_with(values_count, || None);

                let mut acks = Vec::with_capacity(shards_count);
                for (sid, bucket) in buckets.into_iter().enumerate() {
                    if bucket.is_empty() { continue; }
                    let (ack_tx, ack_rx) = crossbeam::channel::bounded(1);
                    let _ = &self.shards[sid].fast_send(WriterCommand::HeadForIndexTagged(bucket, ack_tx))?;
                    acks.push(ack_rx);
                }

                for rx in acks {
                    for (pos, head) in rx.recv()?? {
                        out[pos] = head;
                    }
                }

                Ok(out)
            }
        }
    }

    pub fn insert_kv(&self, key: K, value: V) -> Result<(), AppError> {
        let sid =
            match &self.partitioner {
                Partitioning::ByKey(kp) => kp.partition_key(key.borrow()),
                Partitioning::ByValue(vp) => vp.partition_value(value.borrow()),
            };
        self.shards[sid].insert_kv(key, value)
    }

    pub fn delete_kv(&self, key: K) -> redb::Result<bool, AppError> {
        match &self.partitioner {
            Partitioning::ByKey(kp) => {
                let sid = kp.partition_key(key.borrow());
                self.shards[sid].delete_kv(key)
            },
            Partitioning::ByValue(_) => {
                for w in &self.shards {
                    if w.delete_kv(key.clone())? {
                        return Ok(true);
                    }
                }
                Ok(false)
            },
        }
    }

    pub fn flush(&self) -> redb::Result<(), AppError> {
        for w in &self.shards { w.flush()?; }
        Ok(())
    }

    pub fn flush_async(&self) -> redb::Result<Vec<FlushFuture>, AppError> {
        let mut v = Vec::with_capacity(self.shards.len());
        for w in &self.shards { v.extend(w.flush_async()?) }
        Ok(v)
    }

    pub fn shutdown(self) -> redb::Result<(), AppError> {
        for w in self.shards { w.shutdown()?; }
        Ok(())
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use crate::storage::table_writer_sharded::{Partitioning, ShardedTableWriter};
    use crate::{BytesPartitioner, PlainFactory, ShardedReadOnlyPlainTable};

    #[test]
    fn constructor_compiles() {
        let table_def = redb::TableDefinition::<&[u8], &[u8]>::new("test_table");
        let partitioner = Partitioning::by_key(2);
        let pk_partitioner = BytesPartitioner::new(2);
        let dbs = vec![];
        let writer = ShardedTableWriter::new(partitioner.clone(), dbs.clone(), PlainFactory::new(table_def));
        let reader = ShardedReadOnlyPlainTable::new(pk_partitioner, dbs, table_def);

    }
}