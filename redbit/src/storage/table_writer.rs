use crate::storage::async_boundary::CopyOwnedValue;
use crate::storage::router::{PlainRouter, Router};
use crate::storage::sort_buffer::MergeBuffer;
use crate::storage::table_writer_api::*;
use crate::{error, AppError};
use crossbeam::channel::{bounded, unbounded, Receiver, Sender};
use redb::{Database, Durability, Key};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::{Arc, Weak};
use std::thread;
use std::thread::JoinHandle;
use std::time::Instant;

struct TxState<'txn, 'c, K, V, F>
where
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V>,
{
    table: F::Table<'txn, 'c>,
    async_merge_buf: RefCell<MergeBuffer<K, V>>,
    deferred: Option<FlushState>,
    write_error: Option<AppError>,
    collecting_start: Instant,
}

impl<'txn, 'c, K, V, F> TxState<'txn, 'c, K, V, F>
where
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V>,
{
    fn step(&mut self, cmd: WriterCommand<K, V>) -> Result<Control, AppError> {
        match cmd {
            WriterCommand::WriteInsertNow(k, v) => {
                self.table.insert_kv(k, v)?;
                Ok(Control::Continue)
            }
            WriterCommand::MergeUnsortedInserts(kvs) => {
                self.async_merge_buf.borrow_mut().merge_unsorted(kvs);
                Ok(Control::Continue)
            }
            WriterCommand::AppendSortedInserts(kvs) => {
                self.async_merge_buf.borrow_mut().append_sorted(kvs);
                Ok(Control::Continue)
            }
            WriterCommand::WriteSortedInsertsOnFlush(kvs) => {
                if !self.async_merge_buf.borrow().is_empty() {
                    Err(AppError::Custom("WriteSortedInserts cannot be mixed with SortInserts now".to_string()))
                } else {
                    self.async_merge_buf = RefCell::new(MergeBuffer::from_sorted(kvs));
                    Ok(Control::Continue)
                }
            }
            WriterCommand::Remove(k, ack) => {
                let r = self.table.delete_kv(k)?;
                ack.send(Ok(r))?;
                Ok(Control::Continue)
            }
            WriterCommand::QueryAndWrite { last_shards, values, sink } => {
                if !values.is_empty() || last_shards.is_some() {
                    let mut out = Vec::with_capacity(values.len());
                    for (idx, v) in values.into_iter() {
                        out.push((idx, self.table.get_any_for_index(v)?));
                    }
                    sink(last_shards, out)?;
                }
                Ok(Control::Continue)
            }
            WriterCommand::Range(from, until, ack) => {
                let r = self.table.range(from..until)?;
                ack.send(Ok(r))?;
                Ok(Control::Continue)
            }
            WriterCommand::FlushWhenReady(ack) => {
                match &mut self.deferred {
                    Some(FlushState { sender, .. }) => {
                        if sender.is_some() {
                            return Ok(Control::Error(ack, AppError::Custom("flush already pending".to_string())));
                        }
                        *sender = Some(ack.clone())
                    },
                    None => self.deferred = Some(FlushState { sender: Some(ack.clone()), sum: 0, shards: None }),
                }
                if let Some(FlushState { sender: Some(_), sum, shards: Some(total) }) = &self.deferred {
                    if *sum == *total {
                        return self.step(WriterCommand::Flush(ack));
                    }
                }
                Ok(Control::Continue)
            }
            WriterCommand::ReadyForFlush(total) => {
                match &mut self.deferred {
                    Some(FlushState { sum, shards, .. }) => { *sum += 1; *shards = Some(total); }
                    None => self.deferred = Some(FlushState { sender: None, sum: 1, shards: Some(total) }),
                }
                if let Some(FlushState { sender: Some(ack), sum, shards: Some(t) }) = &self.deferred {
                    if *sum == *t {
                        return self.step(WriterCommand::Flush(ack.clone()));
                    }
                }
                Ok(Control::Continue)
            }

            // --- write (no commit here; we only have &WriteTransaction) ---
            WriterCommand::Flush(sender) => {
                if let Some(error) = self.write_error.take() {
                    return Ok(Control::Error(sender, error));
                }

                let collect_took = self.collecting_start.elapsed().as_millis();

                let mut buf = self.async_merge_buf.borrow_mut();
                let sort_start = Instant::now();
                let kvs = buf.take_sorted();
                let sort_took = sort_start.elapsed().as_millis();

                let write_start = Instant::now();
                if !kvs.is_empty() {
                    if let Err(err) = self.table.insert_many_sorted_by_key(kvs) {
                        buf.clear();
                        return Ok(Control::Commit(sender, Err(err)));
                    }
                }
                let write_took = write_start.elapsed().as_millis();
                buf.clear();
                drop(buf);

                Ok(Control::Commit(sender, Ok(WriteResult::new(collect_took, sort_took, write_took))))
            }

            WriterCommand::Shutdown(ack) => Ok(Control::Shutdown(ack)),
            WriterCommand::Begin(_) => unreachable!("Begin handled outside"),
        }
    }

    fn drain_batch(&mut self, rx: &Receiver<WriterCommand<K, V>>) -> Result<Control, AppError> {
        let mut ctrl = self.step(rx.recv()?)?;
        if !matches!(ctrl, Control::Continue) { return Ok(ctrl); }
        for cmd in rx.try_iter() {
            ctrl = self.step(cmd)?;
            if !matches!(ctrl, Control::Continue) { break; }
        }
        Ok(ctrl)
    }
}

// ========================= TableWriter (outer loop; drop st before moving tx) =========================
pub struct TableWriter<K: CopyOwnedValue + Send + 'static, V: Key + Send + 'static, F> {
    topic: Sender<WriterCommand<K, V>>,
    handle: JoinHandle<()>,
    pub router: Arc<dyn Router<K, V>>,
    sync_buf: RefCell<Vec<(K, V)>>,
    _marker: PhantomData<F>,
}

impl<K, V, F> TableWriter<K, V, F>
where
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V> + Send + Clone + 'static,
{
    pub fn new(db_weak: Weak<Database>, factory: F, durability: Durability) -> Result<Self, AppError> {
        let (topic, receiver): (Sender<WriterCommand<K, V>>, Receiver<WriterCommand<K, V>>) = unbounded();
        let handle = thread::spawn(move || {
            'outer: loop {
                let cmd = match receiver.recv() {
                    Ok(c) => c,
                    Err(e) => { error!("writer {} terminated: {}", factory.name(), e.to_string()); break; }
                };

                match cmd {
                    WriterCommand::Begin(ack) => {
                        let db_arc = match db_weak.upgrade() {
                            Some(db) => db,
                            None => { let _ = ack.send(Err(AppError::Custom("database closed".to_string()))); break 'outer; }
                        };

                        let mut tx = match db_arc.begin_write() {
                            Ok(tx) => tx,
                            Err(e) => { let _ = ack.send(Err(AppError::from(e))); continue 'outer; }
                        };
                        match tx.set_durability(durability) {
                            Ok(()) => {}
                            Err(e) => { let _ = ack.send(Err(AppError::Custom(e.to_string()))); continue 'outer; }
                        }
                        drop(db_arc);

                        let mut cache_local = factory.new_cache();
                        let table = match factory.open(&tx, &mut cache_local) {
                            Ok(t) => { let _ = ack.send(Ok(())); t },
                            Err(e) => { let _ = ack.send(Err(e)); continue 'outer; }
                        };

                        let mut st = TxState::<K, V, F> {
                            table,
                            async_merge_buf: RefCell::new(MergeBuffer::new()),
                            deferred: None,
                            write_error: None,
                            collecting_start: Instant::now(),
                        };

                        'in_tx: loop {
                            match st.drain_batch(&receiver) {
                                Ok(Control::Continue) => continue,
                                Ok(Control::Error(sender, err)) => {
                                    let _ = sender.send(Err(err));
                                    break 'in_tx; // tx drops -> abort
                                }
                                Ok(Control::Commit(sender, result)) => {
                                    match result {
                                        Err(write_err) => {
                                            let _ = sender.send(Err(write_err));
                                            break 'in_tx; // tx drops -> abort
                                        }
                                        Ok(wr) => {
                                            drop(st); // ends &tx borrow
                                            let flush_start = Instant::now();
                                            match tx.commit() {
                                                Ok(()) => {
                                                    let flush_took = flush_start.elapsed().as_millis();
                                                    let stats = TaskStats::new(wr.collect_took, wr.sort_took, wr.write_took, flush_took);
                                                    let _ = sender.send(Ok(TaskResult::new(&factory.name(), stats)));
                                                }
                                                Err(e) => {
                                                    let _ = sender.send(Err(AppError::from(e)));
                                                }
                                            }
                                            break 'in_tx;
                                        }
                                    }
                                }
                                Ok(Control::Shutdown(ack)) => {
                                    if let Some(FlushState { sender: Some(pending), .. }) = st.deferred.take() {
                                        let _ = pending.send(Err(AppError::Custom("aborted".to_string())));
                                    }
                                    drop(st); // drop table borrow first
                                    drop(tx); // abort tx
                                    let _ = ack.send(Ok(()));
                                    break 'outer;
                                }
                                Ok(other) => {
                                    debug_assert!(!matches!(other, Control::Continue));
                                    continue;
                                }
                                Err(err) => {
                                    error!("{} write tx error: {}", factory.name(), err);
                                    st.write_error = Some(err);
                                    continue;
                                }
                            }
                        }
                    }
                    WriterCommand::Shutdown(ack) => {
                        let _ = ack.send(Ok(()));
                        break 'outer;
                    }
                    other => {
                        error!("{} received {:?} outside <Begin - Flush> scope; ignoring", factory.name(), std::mem::discriminant(&other));
                        break 'outer;
                    }
                }
            }
        });

        let router = Arc::new(PlainRouter::new(topic.clone()));
        Ok(Self { topic, router, handle, sync_buf: RefCell::new(Vec::new()), _marker: PhantomData })
    }

    pub fn sender(&self) -> Sender<WriterCommand<K, V>> { self.topic.clone() }
}

impl<K, V, F> WriterLike<K, V> for TableWriter<K, V, F>
where
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V> + Send + Clone + 'static,
{
    fn router(&self) -> Arc<dyn Router<K, V>> {
        self.router.clone()
    }

    fn begin(&self) -> Result<(), AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<(), AppError>>(1);
        self.topic.send(WriterCommand::Begin(ack_tx))?;
        ack_rx.recv()?
    }

    fn begin_async(&self) -> Result<Vec<StartFuture>, AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<(), AppError>>(1);
        self.topic.send(WriterCommand::Begin(ack_tx))?;
        Ok(vec![StartFuture(ack_rx)])
    }

    fn insert_on_flush(&self, key: K, value: V) -> Result<(), AppError> {
        Ok(self.sync_buf.borrow_mut().push((key, value)))
    }

    fn insert_now(&self, key: K, value: V) -> Result<(), AppError> {
        self.router.write_insert_now(key, value)
    }

    fn flush(&self) -> Result<TaskResult, AppError> {
        if !self.sync_buf.borrow().is_empty() {
            self.router.write_sorted_inserts_on_flush(std::mem::take(&mut *self.sync_buf.borrow_mut()))?;
        }
        let (ack_tx, ack_rx) = bounded::<Result<TaskResult, AppError>>(1);
        self.topic.send(WriterCommand::Flush(ack_tx))?;
        ack_rx.recv()?
    }

    fn flush_async(&self) -> Result<Vec<FlushFuture>, AppError> {
        if !self.sync_buf.borrow().is_empty() {
            self.router.write_sorted_inserts_on_flush(std::mem::take(&mut *self.sync_buf.borrow_mut()))?;
        }
        let (ack_tx, ack_rx) = bounded::<Result<TaskResult, AppError>>(1);
        self.topic.send(WriterCommand::Flush(ack_tx))?;
        Ok(vec![FlushFuture::eager(ack_rx)])
    }

    fn flush_two_phased(&self) -> Result<Vec<FlushFuture>, AppError> {
        if !self.sync_buf.borrow().is_empty() {
            self.router.write_sorted_inserts_on_flush(std::mem::take(&mut *self.sync_buf.borrow_mut()))?;
        }
        Ok(vec![FlushFuture::lazy(self.sender())])
    }

    fn flush_deferred(&self) -> Result<Vec<FlushFuture>, AppError> {
        if !self.sync_buf.borrow().is_empty() {
            self.router.write_sorted_inserts_on_flush(std::mem::take(&mut *self.sync_buf.borrow_mut()))?;
        }
        let (ack_tx, ack_rx) = bounded::<Result<TaskResult, AppError>>(1);
        self.topic.send(WriterCommand::FlushWhenReady(ack_tx))?;
        Ok(vec![FlushFuture::eager(ack_rx)])
    }

    fn shutdown(self) -> Result<(), AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<(), AppError>>(1);
        self.topic.send(WriterCommand::Shutdown(ack_tx))?;
        ack_rx.recv()??;
        self.handle.join().map_err(|_| AppError::Custom("Write table join failed".to_string()))?;
        Ok(())
    }

    fn shutdown_async(self) -> Result<Vec<StopFuture>, AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<(), AppError>>(1);
        self.topic.send(WriterCommand::Shutdown(ack_tx))?;
        Ok(vec![StopFuture { ack: ack_rx, handle: self.handle }])
    }
}