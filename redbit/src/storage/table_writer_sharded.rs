use crate::storage::async_boundary::{CopyOwnedValue, ValueOwned};
use crate::storage::partitioning::{KeyPartitioner, Partitioning, ValuePartitioner};
use crate::storage::table_writer::{StopFuture, TableFactory, TaskResult, WriterCommand};
use crate::{AppError, StartFuture, FlushFuture, TableWriter};
use redb::{Database, Key};
use std::borrow::Borrow;
use std::{marker::PhantomData, sync::Weak};
use std::sync::Arc;

pub struct ShardedTableWriter<
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V>,
    KP: KeyPartitioner<K>,
    VP: ValuePartitioner<V>,
> {
    partitioner: Partitioning<KP, VP>,
    shards: Vec<TableWriter<K, V, F>>,
    _pd: PhantomData<(K,V)>,
}

impl<
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V> + Send + Clone + 'static,
    KP: KeyPartitioner<K>,
    VP: ValuePartitioner<V>,
> ShardedTableWriter<K,V,F, KP, VP> {
    pub fn new(partitioning: Partitioning<KP, VP>, dbs: Vec<Weak<Database>>, factory: F) -> Result<Self, AppError> {
        let shards_count = dbs.len();
        if shards_count < 2 {
            return Err(AppError::Custom(format!("ShardedTableWriter: expected at least 2 databases, got {}", shards_count)));
        }
        let mut shards = Vec::with_capacity(shards_count);
        for db_weak in dbs.into_iter() {
            shards.push(TableWriter::<K,V,F>::new(db_weak, factory.clone())?);
        }
        Ok(Self { partitioner: partitioning, shards, _pd: PhantomData })
    }

    pub fn begin(&self) -> Result<(), AppError> {
        for w in &self.shards { w.begin()?; }
        Ok(())
    }

    pub fn begin_async(&self) -> Result<Vec<StartFuture>, AppError> {
        let mut v = Vec::with_capacity(self.shards.len());
        for w in &self.shards { v.extend(w.begin_async()?); }
        Ok(v)
    }

    pub fn get_any_for_index<FN>(&self, values: Vec<V>, then: FN) -> Result<(), AppError>
    where FN: Fn(Vec<(usize, Option<ValueOwned<K>>)>) + Send + Sync + 'static {
        match &self.partitioner {
            Partitioning::ByKey(_) => {
                Err(AppError::Custom("ShardedTableWriter: get_any_for_index not supported with key partitioning".into()))
            }
            Partitioning::ByValue(vp) => {
                let shards_count = self.shards.len();
                let values_count = values.len();
                if values_count == 0 { return Ok(()); }

                let shard_cap = values_count / shards_count;
                let mut buckets: Vec<Vec<(usize, V)>> = (0..shards_count).map(|_| Vec::with_capacity(shard_cap)).collect();

                for (pos, v) in values.into_iter().enumerate() {
                    let sid = vp.partition_value(v.borrow());
                    buckets[sid].push((pos, v));
                }

                let mut out: Vec<Option<ValueOwned<K>>> = Vec::with_capacity(values_count);
                out.resize_with(values_count, || None);

                let then = Arc::new(then);
                for (sid, values) in buckets.into_iter().enumerate() {
                    if values.is_empty() { continue; }
                    let _ = &self.shards[sid].fast_send(WriterCommand::AnyForIndexBucket { values, then: then.clone() })?;
                }
                Ok(())
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

    pub fn delete_kv(&self, key: K) -> Result<bool, AppError> {
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

    pub fn flush(&self) -> redb::Result<TaskResult, AppError> {
        let mut acks = Vec::with_capacity(self.shards.len());
        for w in &self.shards {
            acks.extend(w.flush_async()?);
        }
        let mut tasks = Vec::with_capacity(acks.len());
        for fut in acks {
            tasks.push(fut.wait()?);
        }
        Ok(tasks.into_iter().max_by_key(|t| t.write_took).unwrap())
    }

    pub fn flush_async(&self) -> Result<Vec<FlushFuture>, AppError> {
        let mut v = Vec::with_capacity(self.shards.len());
        for w in &self.shards { v.extend(w.flush_async()?) }
        Ok(v)
    }

    pub fn flush_deferred(&self) -> Vec<FlushFuture> {
        let mut v = Vec::with_capacity(self.shards.len());
        for w in &self.shards { v.extend(w.flush_deferred()) }
        v
    }

    pub fn shutdown(self) -> Result<(), AppError> {
        for w in self.shards { w.shutdown()?; }
        Ok(())
    }
    pub fn shutdown_async(self) -> Result<Vec<StopFuture>, AppError> {
        let mut v = Vec::with_capacity(self.shards.len());
        for w in self.shards { v.extend(w.shutdown_async()?); }
        Ok(v)
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod plain_sharded {
    use crate::impl_copy_owned_value_identity;
    use crate::storage::async_boundary::CopyOwnedValue;
    use crate::storage::test_utils::addr;
    use crate::storage::{plain_test_utils, test_utils};

    impl_copy_owned_value_identity!(u32);
    // insert-only integrity
    #[test]
    fn sharded_plain_insert_read_all() {
        let n = 3usize;
        let name = "plain_sharded_insert";
        let (_owned, weak_dbs) = test_utils::mk_shard_dbs(n, name);
        let (writer, plain_def) = plain_test_utils::mk_sharded_writer(name, n, weak_dbs.clone());

        writer.begin().expect("begin");
        for k in 1u32..=24 {
            let v = addr(&[k as u8, (k + 1) as u8, (k * 3) as u8]);
            writer.insert_kv(k, v).expect("insert");
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

        writer.begin().expect("begin");
        for k in 1u32..=40 {
            writer.insert_kv(k, addr(&[k as u8])).expect("insert");
        }
        for k in (3u32..=40).step_by(3) {
            assert!(writer.delete_kv(k).expect("delete"));
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
    use crate::storage::test_utils::{addr, Address};
    use crate::storage::{index_test_utils, test_utils};
    use crate::storage::async_boundary::ValueOwned;
    use crossbeam::channel;
    use std::time::Duration;

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

        writer.begin().expect("begin");
        // a1 -> {10, 3}
        writer.insert_kv(10u32, a1.clone()).expect("ins");
        writer.insert_kv(3u32,  a1.clone()).expect("ins");
        // a2 -> {7}
        writer.insert_kv(7u32,  a2.clone()).expect("ins");
        // a3 -> {100, 4, 80}
        writer.insert_kv(100u32, a3.clone()).expect("ins");
        writer.insert_kv(80u32,  a3.clone()).expect("ins");
        writer.insert_kv(4u32,   a3.clone()).expect("ins");

        // ----- heads before delete -----
        {
            let want = 3usize; // querying [a1, a2, a3]
            let (tx, rx) = channel::unbounded::<Vec<(usize, Option<ValueOwned<u32>>)>>();

            writer.get_any_for_index(vec![a1.clone(), a2.clone(), a3.clone()], move |batch| {
                // non-blocking: just forward the shard batch
                let _ = tx.send(batch);
            }).expect("enqueue heads_before");

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

        // delete current head for a3 (4) and re-check
        assert!(writer.delete_kv(4u32).expect("del 4"));

        // ----- head after delete (only a3) -----
        {
            let want = 1usize;
            let (tx, rx) = channel::unbounded::<Vec<(usize, Option<ValueOwned<u32>>)>>();

            writer.get_any_for_index(vec![a3.clone()], move |batch| {
                let _ = tx.send(batch);
            }).expect("enqueue head_after");

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

        writer.flush().expect("flush");

        let reader = index_test_utils::mk_sharded_reader::<Address>(n, weak_dbs, pk_by_index_def, index_by_pk_def);
        let keys_iter = reader.get_keys(&a3).expect("get_keys a3");
        let mut keys: Vec<u32> = keys_iter.into_iter().map(|g| g.unwrap().value()).collect();
        keys.sort();
        assert_eq!(keys, vec![80, 100]);

        writer.shutdown().expect("shutdown");
    }

    #[test]
    fn sharded_index_delete_nonexistent_false() {
        let n = 3usize;
        let name = "index_sharded_del_absent";
        let (_owned, weak_dbs) = test_utils::mk_shard_dbs(n, name);
        let (writer, _, _) = index_test_utils::mk_sharded_writer::<Address>(name, n, 1000, weak_dbs.clone());

        writer.begin().expect("begin");
        // no inserts; delete should be false
        assert!(!writer.delete_kv(123456u32).expect("delete absent"));
        writer.flush().expect("flush");
        writer.shutdown().expect("shutdown");
    }

}

#[cfg(all(test, not(feature = "integration")))]
mod dict_sharded {
    use crate::storage::test_utils::addr;
    use crate::storage::{dict_test_utils, test_utils};

    #[test]
    fn sharded_dict_two_ids_same_value_share_after_flush() {
        let n = 4usize;
        let name = "dict_sharded_share";
        let (_owned, weak_dbs) = test_utils::mk_shard_dbs(n, name);
        let (writer, dict_pk_to_ids, value_by_dict_pk, value_to_dict_pk, dict_pk_by_id) = dict_test_utils::mk_sharded_writer(name, n, weak_dbs.clone());

        let id1 = 10u32;
        let id2 = 11u32;
        let val = addr(&[0xDE, 0xAD, 0xBE, 0xEF]);

        writer.begin().expect("begin");
        writer.insert_kv(id1, val.clone()).expect("insert id1");
        writer.insert_kv(id2, val.clone()).expect("insert id2");
        writer.insert_kv(20u32, addr(&[1, 2, 3])).expect("insert other");
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

        writer.begin().expect("begin");
        writer.insert_kv(id1, val.clone()).expect("ins id1");
        writer.insert_kv(id2, val.clone()).expect("ins id2");
        assert!(writer.delete_kv(id1).expect("del id1"));
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