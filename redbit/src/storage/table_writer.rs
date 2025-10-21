use crate::storage::async_boundary::CopyOwnedValue;
use crate::storage::partitioning::{KeyPartitioner, Partitioning, ValuePartitioner};
use crate::storage::router::{Router, ShardedRouter};
use redb::{Database, Durability, Key};
use std::borrow::Borrow;
use std::sync::Arc;
use std::{marker::PhantomData, sync::Weak};
use std::cell::RefCell;
use crossbeam::channel::bounded;
use crate::storage::table_writer_api::*;
use crate::{AppError, TxFSM};

pub struct ShardedTableWriter<
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V>,
    KP: KeyPartitioner<K>,
    VP: ValuePartitioner<V>,
> {
    shards: Vec<TxFSM<K, V, F>>,
    pub router: Arc<dyn Router<K, V>>,
    sync_buf: RefCell<Vec<(K, V)>>,
    _pd: PhantomData<(KP,VP)>,
}

impl<
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V> + Send + Clone + 'static,
    KP: KeyPartitioner<K> + Sync + Send + Clone + 'static,
    VP: ValuePartitioner<V> + Sync + Send + Clone + 'static,
> ShardedTableWriter<K,V,F, KP, VP> {
    pub fn new(partitioning: Partitioning<KP, VP>, dbs: Vec<Weak<Database>>, factory: F) -> Result<Self, AppError> {
        let mut shards = Vec::with_capacity(dbs.len());
        for db_weak in dbs.into_iter() {
            shards.push(TxFSM::<K,V,F>::new(db_weak, factory.clone())?);
        }
        let senders: Vec<_> = shards.iter().map(|w| w.sender()).collect();
        let router = Arc::new(ShardedRouter::new(partitioning.clone(), senders));
        Ok(Self { router, shards, sync_buf: RefCell::new(Vec::new()), _pd: PhantomData })
    }
}

impl<K, V, F, KP, VP> WriterLike<K, V> for ShardedTableWriter<K, V, F, KP, VP>
where
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V> + Send + Clone + 'static,
    KP: KeyPartitioner<K> + Sync + Send + Clone + 'static,
    VP: ValuePartitioner<V> + Sync + Send + Clone + 'static,
{

    fn router(&self) -> Arc<dyn Router<K, V>> {
        self.router.clone()
    }

    fn begin(&self, durability: Durability) -> Result<(), AppError> {
        for w in &self.shards {
            let (ack_tx, ack_rx) = bounded::<Result<(), AppError>>(1);
            w.topic.send(WriterCommand::Begin(ack_tx, durability))?;
            let _ = ack_rx.recv()?;
        }
        Ok(())
    }

    fn begin_async(&self, durability: Durability) -> Result<Vec<StartFuture>, AppError> {
        let mut v = Vec::with_capacity(self.shards.len());
        for w in &self.shards {
            let (ack_tx, ack_rx) = bounded::<Result<(), AppError>>(1);
            w.topic.send(WriterCommand::Begin(ack_tx, durability))?;
            v.push(StartFuture(ack_rx));
        }
        Ok(v)
    }

    fn insert_on_flush(&self, key: K, value: V) -> Result<(), AppError> {
        Ok(self.sync_buf.borrow_mut().push((key, value)))
    }

    fn insert_now(&self, key: K, value: V) -> Result<(), AppError> {
        self.router.write_insert_now(key, value)
    }

    fn flush(&self) -> redb::Result<TaskResult, AppError> {
        let mut acks = Vec::with_capacity(self.shards.len());
        if !self.sync_buf.borrow().is_empty() {
            self.router.write_sorted_inserts_on_flush(std::mem::take(&mut *self.sync_buf.borrow_mut()))?;
        }
        for w in &self.shards {
            let (ack_tx, ack_rx) = bounded::<Result<TaskResult, AppError>>(1);
            w.topic.send(WriterCommand::Flush(ack_tx))?;
            acks.push(FlushFuture::eager(ack_rx));
        }
        let mut tasks = Vec::with_capacity(acks.len());
        for fut in acks {
            tasks.push(fut.wait()?);
        }
        Ok(tasks.into_iter().max_by_key(|t| t.stats.sum()).unwrap())
    }

    fn flush_async(&self) -> Result<Vec<FlushFuture>, AppError> {
        if !self.sync_buf.borrow().is_empty() {
            self.router.write_sorted_inserts_on_flush(std::mem::take(&mut *self.sync_buf.borrow_mut()))?;
        }
        let mut v: Vec<FlushFuture> = Vec::with_capacity(self.shards.len());
        for w in &self.shards {
            let (ack_tx, ack_rx) = bounded::<Result<TaskResult, AppError>>(1);
            w.topic.send(WriterCommand::Flush(ack_tx))?;
            v.push(FlushFuture::eager(ack_rx));
        }
        Ok(v)
    }

    fn flush_two_phased(&self) -> Result<Vec<FlushFuture>, AppError> {
        if !self.sync_buf.borrow().is_empty() {
            self.router.write_sorted_inserts_on_flush(std::mem::take(&mut *self.sync_buf.borrow_mut()))?;
        }
        let mut v: Vec<FlushFuture> = Vec::with_capacity(self.shards.len());
        for w in &self.shards {
            v.push(FlushFuture::lazy(w.sender()))
        }
        Ok(v)
    }

    fn flush_deferred(&self) -> Result<Vec<FlushFuture>, AppError> {
        if !self.sync_buf.borrow().is_empty() {
            self.router.write_sorted_inserts_on_flush(std::mem::take(&mut *self.sync_buf.borrow_mut()))?;
        }
        let mut v = Vec::with_capacity(self.shards.len());
        for w in &self.shards {
            let (ack_tx, ack_rx) = bounded::<Result<TaskResult, AppError>>(1);
            w.topic.send(WriterCommand::FlushWhenReady(ack_tx, 0))?;
            v.push(FlushFuture::eager(ack_rx))
        }
        Ok(v)
    }

    fn shutdown(self) -> Result<(), AppError> {
        for w in self.shards {
            let (ack_tx, ack_rx) = bounded::<Result<(), AppError>>(1);
            w.topic.send(WriterCommand::Shutdown(ack_tx))?;
            ack_rx.recv()??;
            w.handle.join().map_err(|_| AppError::Custom("Write table join failed".to_string()))?;
        }
        Ok(())
    }
    fn shutdown_async(self) -> Result<Vec<StopFuture>, AppError> {
        let mut v = Vec::with_capacity(self.shards.len());
        for w in self.shards {
            let (ack_tx, ack_rx) = bounded::<Result<(), AppError>>(1);
            w.topic.send(WriterCommand::Shutdown(ack_tx))?;
            v.push(StopFuture { ack: ack_rx, handle: w.handle });
        }
        Ok(v)
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod plain_sharded {
    use redb::Durability;
    use crate::impl_copy_owned_value_identity;
    use crate::storage::async_boundary::CopyOwnedValue;
    use crate::storage::test_utils::addr;
    use crate::storage::{plain_test_utils, test_utils};
    use crate::storage::table_writer_api::WriterLike;

    impl_copy_owned_value_identity!(u32);
    // insert-only integrity
    #[test]
    fn sharded_plain_insert_read_all() {
        let n = 3usize;
        let name = "plain_sharded_insert";
        let (_owned, weak_dbs) = test_utils::mk_shard_dbs(n, name);
        let (writer, plain_def) = plain_test_utils::mk_sharded_writer(name, n, weak_dbs.clone());

        writer.begin(Durability::None).expect("begin");
        for k in 1u32..=24 {
            let v = addr(&[k as u8, (k + 1) as u8, (k * 3) as u8]);
            writer.insert_on_flush(k, v).expect("insert");
        }
        writer.flush().expect("flush");

        let reader = plain_test_utils::mk_sharded_reader(n, weak_dbs, plain_def);
        for k in 1u32..=24 {
            let got = reader.get_value(&k).expect("get").expect("some");
            assert_eq!(got.value().0, vec![k as u8, (k + 1) as u8, (k * 3) as u8]);
        }

        writer.shutdown().expect("shutdown");
    }

    // insert + delete (same tx), validate final contents
    #[test]
    fn sharded_plain_delete_every_third() {
        let n = 3usize;
        let name = "plain_sharded_delete";
        let (_owned, weak_dbs) = test_utils::mk_shard_dbs(n, name);
        let (writer, plain_def) = plain_test_utils::mk_sharded_writer(name, n, weak_dbs.clone());

        writer.begin(Durability::None).expect("begin");
        for k in 1u32..=40 {
            writer.insert_on_flush(k, addr(&[k as u8])).expect("insert");
        }
        writer.flush().expect("flush");
        writer.begin(Durability::None).expect("begin");
        for k in (3u32..=40).step_by(3) {
            assert!(writer.router.delete_kv(k).expect("delete"));
        }
        writer.flush().expect("flush");

        let reader = plain_test_utils::mk_sharded_reader(n, weak_dbs, plain_def);
        for k in 1u32..=40 {
            let got = reader.get_value(&k).expect("get");
            if k % 3 == 0 {
                assert!(got.is_none(), "key {} should be deleted", k);
            } else {
                assert!(got.is_some(), "key {} should remain", k);
            }
        }

        writer.shutdown().expect("shutdown");
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod index_sharded {
    use crate::storage::async_boundary::ValueOwned;
    use crate::storage::test_utils::{addr, Address};
    use crate::storage::{index_test_utils, test_utils};
    use crate::storage::table_writer_api::WriterLike;
    use crossbeam::channel;
    use std::sync::Arc;
    use std::time::Duration;
    use redb::Durability;

    #[test]
    fn inserting_in_wrong_order_should_fail() {
        let n = 3usize;
        let name = "index_sharded_heads";
        let (_owned, weak_dbs) = test_utils::mk_shard_dbs(n, name);
        let (writer, _, _) = index_test_utils::mk_sharded_writer::<Address>(name, n, 1000, weak_dbs.clone());
        writer.begin(Durability::None).expect("begin");
        writer.insert_on_flush(4u32, addr(&[0xAA, 0xBB, 0xCC])).expect("ins");
        writer.insert_on_flush(3u32, addr(&[1, 2, 3, 4])).expect("ins");
        let flush_result = writer.flush();
        assert!(flush_result.is_err());
        assert!(format!("{}", flush_result.err().unwrap()).to_lowercase().contains("sorted by key"));
    }

    #[test]
    fn sharded_index_heads_and_delete_in_one_tx() {
        let n = 3usize;
        let name = "index_sharded_heads";
        let (_owned, weak_dbs) = test_utils::mk_shard_dbs(n, name);
        let (writer, pk_by_index_def, index_by_pk_def) = index_test_utils::mk_sharded_writer::<Address>(name, n, 1000, weak_dbs.clone());

        // write + validate heads + delete + validate again — all before flush
        let a1 = addr(&[1, 2, 3, 4]);
        let a2 = addr(&[9, 9, 9]);
        let a3 = addr(&[0xAA, 0xBB, 0xCC]);

        writer.begin(Durability::None).expect("begin");
        writer.insert_on_flush(3u32, a1.clone()).expect("ins");
        writer.insert_on_flush(4u32, a3.clone()).expect("ins");
        writer.insert_on_flush(7u32, a2.clone()).expect("ins");
        writer.insert_on_flush(10u32, a1.clone()).expect("ins");
        writer.insert_on_flush(80u32, a3.clone()).expect("ins");
        writer.insert_on_flush(100u32, a3.clone()).expect("ins");
        writer.flush().expect("flush");
        writer.begin(Durability::None).expect("begin");
        // ----- heads before delete -----
        let router = writer.router.clone();
        {
            let want = 3usize; // querying [a1, a2, a3]
            let (tx, rx) = channel::unbounded::<Vec<(usize, Option<ValueOwned<u32>>)>>();

            router.query_and_write(vec![a1.clone(), a2.clone(), a3.clone()], true, Arc::new(move |_last_shards, batch| {
                // non-blocking: just forward the shard batch
                tx.send(batch)?;
                Ok(())
            })).expect("enqueue heads_before");

            let mut acc: Vec<Option<ValueOwned<u32>>> = vec![None; want];
            let mut filled = 0usize;

            while filled < want {
                let batch = rx.recv_timeout(Duration::from_secs(2)).expect("timeout heads_before");
                for (pos, opt) in batch {
                    if acc[pos].is_none() { filled += 1; }
                    acc[pos] = opt;
                }
            }

            assert_eq!(acc[0].as_ref().map(|v| v.as_value()), Some(3u32));
            assert_eq!(acc[1].as_ref().map(|v| v.as_value()), Some(7u32));
            assert_eq!(acc[2].as_ref().map(|v| v.as_value()), Some(4u32));
        }
        assert!(router.delete_kv(4u32).expect("del 4"));

        // ----- head after delete (only a3) -----
        {
            let want = 1usize;
            let (tx, rx) = channel::unbounded::<Vec<(usize, Option<ValueOwned<u32>>)>>();

            let router_c = writer.router.clone();
            router.query_and_write(vec![a3.clone()], true, Arc::new(move |last_shards, batch| {
                tx.send(batch)?;
                if let Some(last_shards) = last_shards {
                    let _ = router_c.ready_for_flush(last_shards);
                }
                Ok(())
            })).expect("enqueue head_after");

            let mut acc: Vec<Option<ValueOwned<u32>>> = vec![None; want];
            let mut filled = 0usize;

            while filled < want {
                let batch = rx.recv_timeout(Duration::from_secs(2)).expect("timeout head_after");
                for (pos, opt) in batch {
                    if acc[pos].is_none() { filled += 1; }
                    acc[pos] = opt;
                }
            }

            assert_eq!(acc[0].as_ref().map(|v| v.as_value()), Some(80u32));
        }

        writer.flush_deferred().expect("flush");

        let reader = index_test_utils::mk_sharded_reader::<Address>(n, weak_dbs, pk_by_index_def, index_by_pk_def);
        let keys_iter = reader.get_keys(&a3).expect("get_keys a3");
        let mut keys: Vec<u32> = keys_iter.into_iter().map(|g| g.unwrap().value()).collect();
        keys.sort();
        assert_eq!(keys, vec![4, 80, 100]);

        writer.shutdown().expect("shutdown");
    }

    #[test]
    fn sharded_index_delete_nonexistent_false() {
        let n = 3usize;
        let name = "index_sharded_del_absent";
        let (_owned, weak_dbs) = test_utils::mk_shard_dbs(n, name);
        let (writer, _, _) = index_test_utils::mk_sharded_writer::<Address>(name, n, 1000, weak_dbs.clone());

        writer.begin(Durability::None).expect("begin");
        // no inserts; delete should be false
        assert!(!writer.router.delete_kv(123456u32).expect("delete absent"));
        writer.flush().expect("flush");
        writer.shutdown().expect("shutdown");
    }

}

#[cfg(all(test, not(feature = "integration")))]
mod dict_sharded {
    use redb::Durability;
    use crate::storage::test_utils::addr;
    use crate::storage::{dict_test_utils, test_utils};
    use crate::storage::table_writer_api::WriterLike;

    #[test]
    fn sharded_dict_two_ids_same_value_share_after_flush() {
        let n = 4usize;
        let name = "dict_sharded_share";
        let (_owned, weak_dbs) = test_utils::mk_shard_dbs(n, name);
        let (writer, dict_pk_to_ids, value_by_dict_pk, value_to_dict_pk, dict_pk_by_id) = dict_test_utils::mk_sharded_writer(name, n, weak_dbs.clone());

        let id1 = 10u32;
        let id2 = 11u32;
        let val = addr(&[0xDE, 0xAD, 0xBE, 0xEF]);

        writer.begin(Durability::None).expect("begin");
        writer.insert_on_flush(id1, val.clone()).expect("insert id1");
        writer.insert_on_flush(id2, val.clone()).expect("insert id2");
        writer.insert_on_flush(20u32, addr(&[1, 2, 3])).expect("insert other");
        writer.flush().expect("flush");

        let reader = dict_test_utils::mk_sharder_reader(n, weak_dbs, dict_pk_to_ids, value_by_dict_pk, value_to_dict_pk, dict_pk_by_id);
        let a = reader.get_value(&id1).expect("get id1").expect("some").value().0;
        let b = reader.get_value(&id2).expect("get id2").expect("some").value().0;
        assert_eq!(a, val.0);
        assert_eq!(b, val.0);

        // get_keys(value) returns both ids
        let keys_opt = reader.get_keys(&val).expect("get_keys");
        let mut ids = keys_opt.expect("some").into_iter().map(|g| g.unwrap().value()).collect::<Vec<u32>>();
        ids.sort();
        assert_eq!(ids, vec![id1, id2]);

        writer.shutdown().expect("shutdown");
    }

    #[test]
    fn sharded_dict_delete_one_id_keeps_other() {
        let n = 4usize;
        let name = "dict_sharded_delete";
        let (_owned, weak_dbs) = test_utils::mk_shard_dbs(n, name);
        let (writer, dict_pk_to_ids, value_by_dict_pk, value_to_dict_pk, dict_pk_by_id) = dict_test_utils::mk_sharded_writer(name, n, weak_dbs.clone());

        let id1 = 21u32;
        let id2 = 22u32;
        let val = addr(&[7, 7, 7, 7]);

        writer.begin(Durability::None).expect("begin");
        writer.insert_on_flush(id1, val.clone()).expect("ins id1");
        writer.insert_on_flush(id2, val.clone()).expect("ins id2");
        writer.flush().expect("flush");
        writer.begin(Durability::None).expect("begin");
        assert!(writer.router.delete_kv(id1).expect("del id1"));
        writer.flush().expect("flush");

        let reader = dict_test_utils::mk_sharder_reader(n, weak_dbs, dict_pk_to_ids, value_by_dict_pk, value_to_dict_pk, dict_pk_by_id);
        assert!(reader.get_value(&id1).expect("get id1").is_none());
        let got2 = reader.get_value(&id2).expect("get id2").expect("some");
        assert_eq!(got2.value().0, val.0);

        let keys_opt = reader.get_keys(&val).expect("get_keys");
        let ids = keys_opt.expect("some").into_iter().map(|g| g.unwrap().value()).collect::<Vec<u32>>();
        assert_eq!(ids, vec![id2]);

        writer.shutdown().expect("shutdown");
    }
}