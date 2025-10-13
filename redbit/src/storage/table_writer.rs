use crate::storage::async_boundary::CopyOwnedValue;
use crate::storage::router::{PlainRouter, Router};
use crate::storage::sort_buffer::MergeBuffer;
use crate::storage::table_writer_api::*;
use crate::{error, AppError};
use crossbeam::channel::{bounded, unbounded, Receiver, Sender};
use redb::{Database, Key};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::{Arc, Weak};
use std::thread;
use std::thread::JoinHandle;
use std::time::Instant;

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
    fn step<T: WriteTableLike<K, V>>(deferred_sender: &mut Option<FlushState>, err: &mut Option<AppError>, collect_start: Instant, table: &mut T, async_merge_buf: &RefCell<MergeBuffer<K, V>>, cmd: WriterCommand<K, V>) -> Result<Control, AppError> {
        match cmd {
            WriterCommand::WriteInsertNow(k, v) => {
                table.insert_kv(k, v)?;
                Ok(Control::Continue)
            }
            WriterCommand::MergeUnsortedInserts(kvs) => {
                async_merge_buf.borrow_mut().merge_unsorted(kvs);
                Ok(Control::Continue)
            }
            WriterCommand::AppendSortedInserts(kvs) => {
                async_merge_buf.borrow_mut().append_sorted(kvs);
                Ok(Control::Continue)
            }
            WriterCommand::WriteSortedInsertsOnFlush(kvs) => {
                if !async_merge_buf.borrow().is_empty() {
                    Err(AppError::Custom("WriteSortedInserts cannot be mixed with SortInserts now".to_string()))?
                } else {
                    async_merge_buf.swap(&RefCell::new(MergeBuffer::from_sorted(kvs)));
                    Ok(Control::Continue)
                }
            }
            WriterCommand::Remove(k, ack) => {
                let r = table.delete_kv(k)?;
                ack.send(Ok(r))?;
                Ok(Control::Continue)
            }
            WriterCommand::QueryAndWrite { last_shards, values, sink } => {
                if !values.is_empty() || last_shards.is_some() {
                    let mut out = Vec::with_capacity(values.len());
                    for (idx, v) in values.into_iter() {
                        out.push((idx, table.get_any_for_index(v)?));
                    }
                    sink(last_shards, out)?;
                }
                Ok(Control::Continue)
            }
            WriterCommand::Range(from, until, ack) => {
                let r = table.range(from..until)?;
                ack.send(Ok(r))?;
                Ok(Control::Continue)
            }
            WriterCommand::FlushWhenReady(ack) => {
                if let Some(FlushState { sender: _, sum, shards: Some(shards_total) }) = deferred_sender.as_ref() {
                    if *sum == *shards_total {
                        return Self::step(deferred_sender, err, collect_start, table, async_merge_buf, WriterCommand::Flush(ack));
                    }
                }
                Ok(Control::FlushWhenReady(ack))
            }
            WriterCommand::ReadyForFlush(shards_total) => {
                if let Some(FlushState { sender: Some(ack), sum, shards: Some(_) }) = deferred_sender.as_ref() {
                    if *sum + 1 == shards_total {
                        return Self::step(deferred_sender, err, collect_start, table, async_merge_buf, WriterCommand::Flush(ack.clone()));
                    }
                }
                Ok(Control::ReadyForFlush(shards_total))
            }
            WriterCommand::Flush(sender) => {
                if let Some(error) = err.take() {
                    Ok(Control::Error(sender, error))
                } else {
                    let collect_took = collect_start.elapsed().as_millis();
                    let mut buf = async_merge_buf.borrow_mut();
                    let sort_start = Instant::now();
                    let kvs = buf.take_sorted();
                    let sort_took = sort_start.elapsed().as_millis();
                    let write_start = Instant::now();
                    if !kvs.is_empty() {
                        match table.insert_many_sorted_by_key(kvs) {
                            Ok(_) => {}
                            Err(err) => {
                                return Ok(Control::Flush(sender, Err(err)));
                            }
                        }
                    }
                    let write_took = write_start.elapsed().as_millis();
                    buf.clear();
                    Ok(Control::Flush(sender, Ok(WriteResult::new(collect_took, sort_took, write_took))))
                }
            },
            WriterCommand::Shutdown(ack) => Ok(Control::Shutdown(ack)),
            WriterCommand::Begin(_) => unreachable!("Begin handled outside"),
        }
    }

    fn drain_batch<T: WriteTableLike<K, V>>(deferred_sender: &mut Option<FlushState>, err: &mut Option<AppError>, collecting_start: Instant, table: &mut T, async_merge_buf: &RefCell<MergeBuffer<K, V>>, rx: &Receiver<WriterCommand<K, V>>) -> Result<Control, AppError> {
        // 1) one blocking recv to ensure progress
        let mut ctrl = Self::step(deferred_sender, err, collecting_start, table, async_merge_buf, rx.recv()?)?;
        if !matches!(ctrl, Control::Continue) {
            return Ok(ctrl);
        }

        // 2) opportunistically drain the channel without blocking
        for cmd in rx.try_iter() {
            ctrl = Self::step(deferred_sender, err, collecting_start, table, async_merge_buf, cmd)?;
            if !matches!(ctrl, Control::Continue) {
                break;
            }
        }
        Ok(ctrl)
    }

    pub fn new(db_weak: Weak<Database>, factory: F) -> Result<Self, AppError> {
        let (topic, receiver): (Sender<WriterCommand<K, V>>, Receiver<WriterCommand<K, V>>) = unbounded();
        let handle = thread::spawn(move || {
            let mut cache = factory.new_cache();
            let mut async_merge_buf = RefCell::new(MergeBuffer::new());
            let name = factory.name();
            'outer: loop {
                // wait until someone asks us to begin a write tx
                let cmd = match receiver.recv() {
                    Ok(c) => c,
                    Err(e) => { error!("writer {} terminated: {}", name, e.to_string()); break; }
                };

                match cmd {
                    WriterCommand::Begin(ack) => {
                        let db_arc = match db_weak.upgrade() {
                            Some(db) => db,
                            None => {
                                let _ = ack.send(Err(AppError::Custom("database closed".to_string())));
                                break 'outer;
                            }
                        };
                        if !async_merge_buf.borrow().is_empty() {
                            let _ = ack.send(Err(AppError::Custom("Begin received while previous transaction not finished".to_string())));
                            break 'outer;
                        }
                        // 0) open a new write tx
                        let tx = match db_arc.begin_write() {
                            Ok(tx) => tx,
                            Err(e) => { let _ = ack.send(Err(AppError::from(e))); continue 'outer; }
                        };
                        // 1) drop the strong Arc immediately; owner keeps DB alive
                        drop(db_arc);

                        // local state
                        let mut write_error: Option<AppError> = None;
                        let mut flush_result: Option<(Sender<Result<TaskResult, AppError>>, Result<WriteResult, AppError>)> = None;
                        let mut deferred_flush_sender: Option<FlushState> = None;

                        // 2) open typed table bound to &tx
                        let mut table = match factory.open(&tx, &mut cache) {
                            Ok(t) => { let _ = ack.send(Ok(())); t },
                            Err(e) => { let _ = ack.send(Err(e)); continue 'outer; }
                        };
                        let collecting_start = Instant::now();

                        // 3) process commands until a Flush arrives
                        'in_tx: loop {
                            match Self::drain_batch(&mut deferred_flush_sender, &mut write_error, collecting_start, &mut table, &mut async_merge_buf, &receiver) {
                                Ok(Control::Continue) => continue,
                                Ok(Control::ReadyForFlush(shards)) => {
                                    match deferred_flush_sender {
                                        Some(FlushState { sender, sum, shards: _}) => deferred_flush_sender = Some(FlushState { sender, sum: sum + 1, shards: Some(shards)}),
                                        None => deferred_flush_sender = Some(FlushState { sender: None, sum: 1, shards: Some(shards)}),
                                    }
                                    continue;
                                },
                                Ok(Control::FlushWhenReady(ack)) => {
                                    match deferred_flush_sender {
                                        Some(FlushState { sender: _, sum, shards}) => deferred_flush_sender = Some(FlushState { sender: Some(ack), sum, shards}),
                                        None => deferred_flush_sender = Some(FlushState { sender: Some(ack), sum: 0, shards: None}),
                                    }
                                },
                                Ok(Control::Error(sender, err)) => {
                                    let _ = sender.send(Err(err));
                                    break 'in_tx;
                                }
                                Ok(Control::Flush(sender, result)) => {
                                    flush_result = Some((sender, result));
                                    break 'in_tx
                                },
                                Ok(Control::Shutdown(ack)) => {
                                    drop(table);
                                    drop(tx);
                                    let _ = ack.send(Ok(()));
                                    break 'outer;
                                }
                                Err(err) => {
                                    error!("{} write tx error: {}", name, err);
                                    write_error = Some(err);
                                    continue;
                                }
                            }
                        }

                        // 4) end-of-tx: drop table FIRST, then commit
                        drop(table);
                        if let Some((sender, result)) = flush_result {
                            match result {
                                Err(err) => {
                                    let _ = sender.send(Err(err));
                                },
                                Ok(s) => {
                                    let flush_start = Instant::now();
                                    match tx.commit() {
                                        Ok(_) => {
                                            let flush_took = flush_start.elapsed().as_millis();
                                            let stats = TaskStats::new(s.collect_took, s.sort_took, s.write_took, flush_took);
                                            let _ = sender.send(Ok(TaskResult::new(&name, stats)));
                                        }
                                        Err(e) => {
                                            let _ = sender.send(Err(AppError::from(e)));
                                        }
                                    }
                                }
                            }
                        } else {
                           error!("Transaction of {} ended without Flush or Shutdown, it can never happen", name);
                        }
                        // go back to idle and wait for next Begin
                    }

                    WriterCommand::Shutdown(ack) => {
                        // no active tx at this point; stop thread
                        let _ = ack.send(Ok(()));
                        break 'outer;
                    }

                    other => {
                        error!("{} received {:?} outside <Begin - Flush> scope; ignoring", name, std::mem::discriminant(&other));
                        break 'outer;
                    }
                }
            }
        });
        let router = Arc::new(PlainRouter::new(topic.clone()));
        Ok(Self { topic, router, handle, sync_buf: RefCell::new(Vec::new()), _marker: PhantomData })
    }

    pub fn sender(&self) -> Sender<WriterCommand<K, V>> {
        self.topic.clone()
    }
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