use crossbeam::channel::{bounded, Sender, TrySendError};
use redb::{Key, Value};
use std::borrow::Borrow;
use std::sync::Arc;

use crate::storage::async_boundary::{ValueBuf, ValueOwned};
use crate::storage::table_writer_api::WriterCommand;
use crate::{AppError, DbKey, KeyPartitioner, Partitioning, DbVal, ValuePartitioner};

#[inline]
fn fast_send<K: DbKey + Send, V: Key + Send + 'static>(
    tx: &Sender<WriterCommand<K, V>>,
    msg: WriterCommand<K, V>
) -> Result<(), AppError> {
    match tx.try_send(msg) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(m)) => tx.send(m).map_err(|e| AppError::Custom(e.to_string())),
        Err(TrySendError::Disconnected(_)) => Err(AppError::Custom("writer thread disconnected".into())),
    }
}

pub trait Router<K: DbKey, V: Value>: Send + Sync {
    fn shards(&self) -> usize;
    fn merge_unsorted_inserts(&self, pairs: Vec<(K, V)>, last_shards: Option<usize>) -> Result<(), AppError>;
    fn ready_for_flush(&self, shards: usize) -> Result<(), AppError>;
    fn write_sorted_inserts_on_flush(&self, pairs: Vec<(K, V)>) -> Result<(), AppError>;
    fn write_insert_now(&self, k: K, v: V) -> Result<(), AppError>;
    fn delete_kv(&self, key: K) -> Result<bool, AppError>;
    fn range(&self, from: K, until: K) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>;
    fn query_and_write(
        &self,
        values: Vec<V>,
        is_last: bool,
        sink: Arc<dyn Fn(Option<usize>, Vec<(usize, Option<ValueOwned<K>>)>) -> Result<(), AppError> + Send + Sync + 'static>,
    ) -> Result<(), AppError>;
}

pub struct ShardedRouter<K: DbKey + Send, V: Key + Send + 'static, KP, VP> {
    part: Partitioning<KP, VP>,
    senders: Arc<[Sender<WriterCommand<K, V>>]>,
}

impl<K, V, KP, VP> ShardedRouter<K, V, KP, VP>
where
    K: DbKey + Send,
    V: DbVal + Send,
    KP: KeyPartitioner<K>,
    VP: ValuePartitioner<V>,
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
    #[inline]
    fn bucket_one<'k, 'v, KB: Borrow<K::SelfType<'k>>, VB: Borrow<V::SelfType<'v>>>(&self, k: KB, v: VB) -> (usize, KB, VB) {
        let sid = match &self.part {
            Partitioning::ByKey(kp)   => kp.partition_key(k.borrow()),
            Partitioning::ByValue(vp) => vp.partition_value(v.borrow()),
        };
        (sid, k, v)
    }
}

impl<K, V, KP, VP> Router<K, V> for ShardedRouter<K, V, KP, VP>
where
    K: DbKey + Send,
    V: DbVal + Send,
    KP: KeyPartitioner<K>,
    VP: ValuePartitioner<V>,
{
    fn shards(&self) -> usize {
        self.senders.len()
    }

    fn merge_unsorted_inserts(&self, pairs: Vec<(K, V)>, last_shards: Option<usize>) -> Result<(), AppError> {
        if !pairs.is_empty() {
            if self.senders.len() == 1 {
                fast_send(&self.senders[0], WriterCommand::MergeUnsortedInserts(pairs))?;
            } else {
                for (sid, bucket) in self.bucket(pairs).into_iter().enumerate() {
                    if bucket.is_empty() { continue; }
                    fast_send(&self.senders[sid], WriterCommand::MergeUnsortedInserts(bucket))?;
                }
            }
        }
        if let Some(from_shards) = last_shards {
            self.ready_for_flush(from_shards)?
        }
        Ok(())
    }

    fn ready_for_flush(&self, shards: usize) -> Result<(), AppError> {
        for s in self.senders.iter() {
            fast_send(s, WriterCommand::ReadyForFlush(shards))?;
        }
        Ok(())
    }

    fn write_sorted_inserts_on_flush(&self, pairs: Vec<(K, V)>) -> Result<(), AppError> {
        if pairs.is_empty() {
            Ok(())
        } else if self.senders.len() == 1 {
            fast_send(&self.senders[0], WriterCommand::WriteSortedInsertsOnFlush(pairs))
        } else {
            for (sid, bucket) in self.bucket(pairs).into_iter().enumerate() {
                if bucket.is_empty() { continue; }
                fast_send(&self.senders[sid], WriterCommand::WriteSortedInsertsOnFlush(bucket))?;
            }
            Ok(())
        }
    }

    fn write_insert_now(&self, k: K, v: V) -> Result<(), AppError> {
        if self.senders.len() == 1 {
            fast_send(&self.senders[0], WriterCommand::WriteInsertNow(k, v))
        } else {
            let (sid, key, value) = self.bucket_one(k, v);
            fast_send(&self.senders[sid], WriterCommand::WriteInsertNow(key, value))
        }
    }

    fn delete_kv(&self, key: K) -> Result<bool, AppError> {
        if self.senders.len() == 1 {
            let (ack_tx, ack_rx) = bounded::<Result<bool, AppError>>(1);
            fast_send(&self.senders[0], WriterCommand::Remove(key, ack_tx))?;
            ack_rx.recv()?
        } else {
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
    }

    fn range(&self, from: K, until: K) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError> {
        if self.senders.len() == 1 {
            let (ack_tx, ack_rx) = bounded::<Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>>(1);
            fast_send(&self.senders[0], WriterCommand::Range(from, until, ack_tx))?;
            ack_rx.recv()?
        } else {
            unimplemented!()
        }
    }

    fn query_and_write(
        &self,
        values: Vec<V>,
        is_last: bool,
        sink: Arc<dyn Fn(Option<usize>, Vec<(usize, Option<ValueOwned<K>>)>) -> Result<(), AppError> + Send + Sync + 'static>,
    ) -> Result<(), AppError> {
        if self.senders.len() == 1 {
            let last_shards = if is_last { Some(1) } else { None };
            fast_send(&self.senders[0], WriterCommand::QueryAndWrite { last_shards, values: values.into_iter().enumerate().collect(), sink })
        } else {
            let vp = match &self.part {
                Partitioning::ByValue(vp) => vp,
                Partitioning::ByKey(_) => {
                    return Err(AppError::Custom("get_any_for_index requires value partitioning".into()));
                }
            };
            if values.is_empty() && !is_last {
                return Ok(());
            }
            let shards = self.shards();
            let mut buckets: Vec<Vec<(usize, V)>> = (0..shards).map(|_| Vec::new()).collect();
            for (pos, v) in values.into_iter().enumerate() {
                let sid = vp.partition_value(v.borrow());
                buckets[sid].push((pos, v));
            }
            let last_shards = if is_last { Some(shards) } else { None };

            for (sid, values) in buckets.into_iter().enumerate() {
                if values.is_empty() && !is_last { continue; }
                fast_send(&self.senders[sid], WriterCommand::QueryAndWrite { last_shards, values, sink: sink.clone() })?;
            }
            Ok(())
        }
    }
}
