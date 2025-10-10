use crossbeam::channel::{bounded, Sender, TrySendError};
use std::sync::Arc;
use std::borrow::Borrow;
use redb::{Key, Value};

use crate::{AppError, CopyOwnedValue, KeyPartitioner, Partitioning, ValuePartitioner};
use crate::storage::async_boundary::{ValueBuf, ValueOwned};
use crate::storage::table_writer_api::WriterCommand;

#[inline]
fn fast_send<K: CopyOwnedValue + Send + 'static, V: Key + Send + 'static>(
    tx: &Sender<WriterCommand<K, V>>,
    msg: WriterCommand<K, V>
) -> Result<(), AppError> {
    match tx.try_send(msg) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(m)) => tx.send(m).map_err(|e| AppError::Custom(e.to_string())),
        Err(TrySendError::Disconnected(_)) => Err(AppError::Custom("writer thread disconnected".into())),
    }
}

pub struct PlainRouter<K: CopyOwnedValue + Send + 'static, V: Key + Send + 'static> {
    sender: Sender<WriterCommand<K, V>>,
}

impl<K: CopyOwnedValue + Send + 'static, V: Key + Send + 'static> PlainRouter<K, V> {
    pub fn new(sender: Sender<WriterCommand<K, V>>) -> Self { Self { sender } }
}

pub trait Router<K: CopyOwnedValue, V: Value>: Send + Sync {
    fn append_sorted_inserts(&self, pairs: Vec<(K, V)>) -> Result<(), AppError>;
    fn merge_unsorted_inserts(&self, pairs: Vec<(K, V)>) -> Result<(), AppError>;
    fn write_sorted_inserts(&self, pairs: Vec<(K, V)>) -> Result<(), AppError>;
    fn delete_kv(&self, key: K) -> Result<bool, AppError>;
    fn range(&self, from: K, until: K) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>;
    fn query_and_write(
        &self,
        values: Vec<V>,
        sink: Arc<dyn Fn(Vec<(usize, Option<ValueOwned<K>>)>) -> Result<(), AppError> + Send + Sync + 'static>,
    ) -> Result<(), AppError>;
}

impl<K, V> Router<K, V> for PlainRouter<K, V>
where
    K: CopyOwnedValue + Send + 'static,
    V: Key + Send + 'static,
{
    fn append_sorted_inserts(&self, pairs: Vec<(K, V)>) -> Result<(), AppError> {
        if pairs.is_empty() { return Ok(()); }
        fast_send(&self.sender, WriterCommand::AppendSortedInserts(pairs))
    }

    fn merge_unsorted_inserts(&self, pairs: Vec<(K, V)>) -> Result<(), AppError> {
        if pairs.is_empty() { return Ok(()); }
        fast_send(&self.sender, WriterCommand::MergeUnsortedInserts(pairs))
    }

    fn write_sorted_inserts(&self, pairs: Vec<(K, V)>) -> Result<(), AppError> {
        if pairs.is_empty() { return Ok(()); }
        fast_send(&self.sender, WriterCommand::WriteSortedInserts(pairs))
    }

    fn delete_kv(&self, key: K) -> Result<bool, AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<bool, AppError>>(1);
        fast_send(&self.sender, WriterCommand::Remove(key, ack_tx))?;
        ack_rx.recv()?
    }

    fn range(&self, from: K, until: K) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>>(1);
        fast_send(&self.sender, WriterCommand::Range(from, until, ack_tx))?;
        ack_rx.recv()?
    }

    fn query_and_write(
        &self,
        values: Vec<V>,
        sink: Arc<dyn Fn(Vec<(usize, Option<ValueOwned<K>>)>) -> Result<(), AppError> + Send + Sync + 'static>,
    ) -> Result<(), AppError> {
        fast_send(&self.sender, WriterCommand::QueryAndWrite { values, sink })
    }
}

// ------------------------ Sharded router ------------------------

pub struct ShardedRouter<K: CopyOwnedValue + Send + 'static, V: Key + Send + 'static, KP, VP> {
    part: Partitioning<KP, VP>,
    senders: Arc<[Sender<WriterCommand<K, V>>]>,
}

impl<K, V, KP, VP> ShardedRouter<K, V, KP, VP>
where
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    KP: KeyPartitioner<K> + Send + Sync + Clone + 'static,
    VP: ValuePartitioner<V> + Send + Sync + Clone + 'static,

{
    pub fn new(part: Partitioning<KP, VP>, senders: Vec<Sender<WriterCommand<K, V>>>) -> Self {
        Self { part, senders: senders.into() }
    }

    #[inline]
    fn shards(&self) -> usize { self.senders.len() }

    fn bucket(&self, pairs: Vec<(K, V)>) -> Vec<Vec<(K, V)>> {
        let n = self.shards();
        let per = (pairs.len() / n).saturating_add(1);
        let mut buckets: Vec<Vec<(K, V)>> = (0..n).map(|_| Vec::with_capacity(per)).collect();

        match &self.part {
            Partitioning::ByKey(kp) => {
                for (k, v) in pairs {
                    let sid = kp.partition_key(k.borrow());
                    buckets[sid].push((k, v));
                }
            }
            Partitioning::ByValue(vp) => {
                for (k, v) in pairs {
                    let sid = vp.partition_value(v.borrow());
                    buckets[sid].push((k, v));
                }
            }
        }
        buckets
    }
}

impl<K, V, KP, VP> Router<K, V> for ShardedRouter<K, V, KP, VP>
where
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    KP: KeyPartitioner<K> + Send + Sync + Clone + 'static,
    VP: ValuePartitioner<V> + Send + Sync + Clone + 'static,
{
    fn append_sorted_inserts(&self, pairs: Vec<(K, V)>) -> Result<(), AppError> {
        for (sid, bucket) in self.bucket(pairs).into_iter().enumerate() {
            if bucket.is_empty() { continue; }
            fast_send(&self.senders[sid], WriterCommand::AppendSortedInserts(bucket))?;
        }
        Ok(())
    }

    fn merge_unsorted_inserts(&self, pairs: Vec<(K, V)>) -> Result<(), AppError> {
        for (sid, bucket) in self.bucket(pairs).into_iter().enumerate() {
            if bucket.is_empty() { continue; }
            fast_send(&self.senders[sid], WriterCommand::MergeUnsortedInserts(bucket))?;
        }
        Ok(())
    }

    fn write_sorted_inserts(&self, pairs: Vec<(K, V)>) -> Result<(), AppError> {
        for (sid, bucket) in self.bucket(pairs).into_iter().enumerate() {
            if bucket.is_empty() { continue; }
            fast_send(&self.senders[sid], WriterCommand::WriteSortedInserts(bucket))?;
        }
        Ok(())
    }

    fn delete_kv(&self, key: K) -> Result<bool, AppError> {
        match &self.part {
            Partitioning::ByKey(kp) => {
                let sid = kp.partition_key(key.borrow());
                let (ack_tx, ack_rx) = bounded::<Result<bool, AppError>>(1);
                fast_send(&self.senders[sid], WriterCommand::Remove(key, ack_tx))?;
                ack_rx.recv()?
            },
            Partitioning::ByValue(_) => {
                for s in self.senders.iter() {
                    let (ack_tx, ack_rx) = bounded::<Result<bool, AppError>>(1);
                    fast_send(s, WriterCommand::Remove(key, ack_tx))?;
                    if ack_rx.recv()?? {
                        return Ok(true);
                    }
                }
                Ok(false)
            },
        }

    }

    fn range(&self, _from: K, _until: K) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError> {
        unimplemented!()
    }

    fn query_and_write(
        &self,
        values: Vec<V>,
        sink: Arc<dyn Fn(Vec<(usize, Option<ValueOwned<K>>)>) -> Result<(), AppError> + Send + Sync + 'static>,
    ) -> Result<(), AppError> {
        let vp = match &self.part {
            Partitioning::ByValue(vp) => vp,
            Partitioning::ByKey(_) => {
                return Err(AppError::Custom("get_any_for_index requires value partitioning".into()));
            }
        };

        let n = self.shards();
        if values.is_empty() { return Ok(()); }

        let mut buckets: Vec<Vec<(usize, V)>> = (0..n).map(|_| Vec::new()).collect();
        for (pos, v) in values.into_iter().enumerate() {
            let sid = vp.partition_value(v.borrow());
            buckets[sid].push((pos, v));
        }

        for (sid, vals) in buckets.into_iter().enumerate() {
            if vals.is_empty() { continue; }
            fast_send(&self.senders[sid], WriterCommand::QueryAndWriteBucket { values: vals, sink: sink.clone() })?;
        }
        Ok(())
    }
}
