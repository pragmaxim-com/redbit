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
    fn step<T: WriteTableLike<K, V>>(table: &mut T, async_merge_buf: &RefCell<MergeBuffer<K, V>>, cmd: WriterCommand<K, V>) -> Result<Control, AppError> {
        match cmd {
            WriterCommand::SortInserts(kvs) => {
                async_merge_buf.borrow_mut().push_unsorted(kvs);
                Ok(Control::Continue)
            }
            WriterCommand::WriteSortedInserts(kvs) => {
                if !async_merge_buf.borrow().is_empty() {
                    Err(AppError::Custom("WriteSortedInserts cannot be mixed with SortInserts now".to_string()))?
                }
                async_merge_buf.swap(&RefCell::new(MergeBuffer::from_sorted(kvs)));
                Ok(Control::Continue)
            }
            WriterCommand::Remove(k, ack) => {
                let r = table.delete_kv(k)?;
                ack.send(Ok(r))?;
                Ok(Control::Continue)
            }
            WriterCommand::QueryAndWrite { values, sink } => {
                let mut out = Vec::with_capacity(values.len());
                for (idx, v) in values.into_iter().enumerate() {
                    out.push((idx, table.get_any_for_index(v)?));
                }
                sink(out)?;
                Ok(Control::Continue)
            }
            WriterCommand::QueryAndWriteBucket { values, sink } => {
                let mut out = Vec::with_capacity(values.len());
                for (idx, v) in values.into_iter() {
                    out.push((idx, table.get_any_for_index(v)?));
                }
                sink(out)?;
                Ok(Control::Continue)
            }
            WriterCommand::Range(from, until, ack) => {
                let r = table.range(from..until)?;
                ack.send(Ok(r))?;
                Ok(Control::Continue)
            }
            WriterCommand::IsReadyForWriting(ack) => {
                Ok(Control::IsReadyForWriting(ack))
            },
            WriterCommand::Flush(ack) => {
                let mut buf = async_merge_buf.borrow_mut();
                let kvs = buf.take_sorted();
                if !kvs.is_empty() {
                    table.insert_many_kvs(kvs,false)?;
                }
                buf.clear();
                Ok(Control::Flush(ack))
            },
            WriterCommand::Shutdown(ack) => Ok(Control::Shutdown(ack)),
            WriterCommand::Begin(_) => unreachable!("Begin handled outside"),
        }
    }

    fn drain_batch<T: WriteTableLike<K, V>>(table: &mut T, async_merge_buf: &RefCell<MergeBuffer<K, V>>, rx: &Receiver<WriterCommand<K, V>>) -> Result<Control, AppError> {
        // 1) one blocking recv to ensure progress
        let mut ctrl = Self::step(table, async_merge_buf, rx.recv()?)?;
        if !matches!(ctrl, Control::Continue) {
            return Ok(ctrl);
        }

        // 2) opportunistically drain the channel without blocking
        for cmd in rx.try_iter() {
            ctrl = Self::step(table, async_merge_buf, cmd)?;
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
                        let mut flush_ack: Option<Sender<Result<TaskResult, AppError>>> = None;
                        let mut write_error: Option<Result<(), AppError>> = None;
                        let write_start = Instant::now();

                        // 2) open typed table bound to &tx
                        let mut table = match factory.open(&tx, &mut cache) {
                            Ok(t) => { let _ = ack.send(Ok(())); t },
                            Err(e) => { let _ = ack.send(Err(e)); continue 'outer; }
                        };

                        // 3) process commands until a Flush arrives
                        'in_tx: loop {
                            match Self::drain_batch(&mut table, &mut async_merge_buf, &receiver) {
                                Ok(Control::Continue) => continue,
                                Ok(Control::IsReadyForWriting(ack)) => { let _ = ack.send(Ok(())); continue; },
                                Ok(Control::Flush(ack)) => { flush_ack = Some(ack); break 'in_tx; }
                                Ok(Control::Shutdown(ack)) => {
                                    drop(table);
                                    drop(tx);
                                    let _ = ack.send(Ok(()));
                                    break 'outer;
                                }
                                Err(err) => {
                                    if write_error.is_none() { write_error = Some(Err(err)); }
                                    break 'in_tx;
                                }
                            }
                        }

                        // 4) end-of-tx: drop table FIRST, then commit
                        drop(table);
                        if let Some(ack) = flush_ack {
                            let _ = match write_error {
                                Some(Err(e)) => ack.send(Err(e)),
                                _ => {
                                    let write_took = write_start.elapsed().as_millis();
                                    let commit_start = Instant::now();
                                    let _ = tx.commit().map_err(AppError::from);
                                    let commit_took = commit_start.elapsed().as_millis();
                                    ack.send(Ok(TaskResult::new(&name, write_took, commit_took)))
                                },
                            };
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
                        error!("{} received {:?} before Begin; ignoring", name, std::mem::discriminant(&other));
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

    fn insert_kv(&self, key: K, value: V) -> Result<(), AppError> {
        Ok(self.sync_buf.borrow_mut().push((key, value)))
    }

    fn flush(&self) -> Result<TaskResult, AppError> {
        self.router.write_sorted_inserts(std::mem::take(&mut *self.sync_buf.borrow_mut()))?;
        let (ack_tx, ack_rx) = bounded::<Result<TaskResult, AppError>>(1);
        self.topic.send(WriterCommand::Flush(ack_tx))?;
        ack_rx.recv()?
    }

    fn flush_async(&self) -> Result<Vec<FlushFuture>, AppError> {
        self.router.write_sorted_inserts(std::mem::take(&mut *self.sync_buf.borrow_mut()))?;
        let (ack_tx, ack_rx) = bounded::<Result<TaskResult, AppError>>(1);
        self.topic.send(WriterCommand::Flush(ack_tx))?;
        Ok(vec![FlushFuture::eager(ack_rx)])
    }

    fn flush_two_phased(&self) -> Vec<FlushFuture> {
        let _ = self.router.write_sorted_inserts(std::mem::take(&mut *self.sync_buf.borrow_mut()));
        vec![FlushFuture::lazy(self.sender())]
    }

    fn flush_three_phased(&self) -> Vec<FlushFuture> {
        let _ = self.router.write_sorted_inserts(std::mem::take(&mut *self.sync_buf.borrow_mut()));
        vec![FlushFuture::ready_and_fire(self.sender(), self.sender())]
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