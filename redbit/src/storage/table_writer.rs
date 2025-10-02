use crate::{error, AppError};
use crossbeam::channel::{bounded, unbounded, Receiver, Sender, TrySendError};
use redb::{AccessGuard, Database, Key, Value, WriteTransaction};
use std::borrow::Borrow;
use std::marker::PhantomData;
use std::ops::RangeBounds;
use std::sync::Weak;
use std::{fmt, thread};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::thread::JoinHandle;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct TaskResult {
    pub name: String,
    pub took: u128,
}

impl fmt::Display for TaskResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} : {} ms", self.name, self.took)
    }
}

impl TaskResult {
    pub fn new(name: &str, took: u128) -> Self {
        Self { name: name.to_string(), took }
    }
    pub fn master(took: u128) -> Self {
        Self { name: "MASTER".to_string(), took }
    }
    pub fn commit(took: u128) -> Self {
        Self { name: "COMMIT".to_string(), took }
    }
}

pub struct FlushFuture {
    ack_rx: Receiver<Result<TaskResult, AppError>>,
}

impl FlushFuture {
    pub fn wait(self) -> Result<TaskResult, AppError> {
        self.ack_rx.recv()?
    }

    pub fn dedup_tasks_keep_slowest(futs: Vec<FlushFuture>) -> Result<HashMap<String, TaskResult>, AppError> {
        let mut by_name: HashMap<String, TaskResult> = HashMap::with_capacity(futs.len());
        for f in futs {
            let res = f.wait()?;
            match by_name.entry(res.name.clone()) {
                Entry::Vacant(e) => { e.insert(res); }
                Entry::Occupied(mut e) => {
                    if res.took > e.get().took {
                        e.insert(res); // keep the slowest per name
                    }
                }
            }
        }
        Ok(by_name)
    }
}

pub struct ValueBuf<V: Value> {
    pub buf: Vec<u8>,
    _pd: PhantomData<V>,
}

impl<V: Value> ValueBuf<V> {
    pub fn new(buf: Vec<u8>) -> Self { Self { buf, _pd: PhantomData } }
    pub fn as_value(&self) -> V::SelfType<'_> { V::from_bytes(&self.buf) }
    pub fn as_bytes(&self) -> &[u8] { &self.buf }
}

pub trait WriteTableLike<K: Key + 'static, V: Key + 'static> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<(), AppError>;
    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> Result<bool, AppError>;
    fn get_any_for_index<'v>(&mut self, value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueBuf<K>>, AppError>;
    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, range: impl RangeBounds<KR> + 'a) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>;

    fn key_buf(g: AccessGuard<'_, K>) -> ValueBuf<K> {
        ValueBuf::<K>::new(K::as_bytes(&g.value()).as_ref().to_vec())
    }
    fn value_buf(g: AccessGuard<'_, V>) -> ValueBuf<V> {
        ValueBuf::<V>::new(V::as_bytes(&g.value()).as_ref().to_vec())
    }
}

pub trait TableFactory<K: Key + 'static, V: Key + 'static> {
    type CacheCtx;
    type Table<'txn, 'c>: WriteTableLike<K, V>;
    fn name(&self) -> String;
    fn new_cache(&self) -> Self::CacheCtx;
    fn open<'txn, 'c>(&self, tx: &'txn WriteTransaction, cache: &'c mut Self::CacheCtx) -> Result<Self::Table<'txn, 'c>, AppError>;
}

pub enum WriterCommand<K: Key + Send + 'static, V: Key + Send + 'static> {
    Begin(Sender<Result<(), AppError>>),              // start new WriteTransaction + open table
    Insert(K, V),
    Remove(K, Sender<Result<bool, AppError>>),
    AnyForIndex(Vec<V>, Sender<Result<Vec<Option<ValueBuf<K>>>, AppError>>),
    AnyForIndexTagged(Vec<(usize, V)>, Sender<Result<Vec<(usize, Option<ValueBuf<K>>)>, AppError>>),
    Range(K, K, Sender<Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>>),
    Flush(Sender<Result<TaskResult, AppError>>),              // commit current tx, stay alive (idle)
    Shutdown(Sender<Result<(), AppError>>),           // graceful stop (no commit)
}

enum Control {
    Continue,
    Flush(Sender<Result<TaskResult, AppError>>),
    Shutdown(Sender<Result<(), AppError>>),
}

pub struct TableWriter<K: Key + Send + 'static, V: Key + Send + 'static, F> {
    topic: Sender<WriterCommand<K, V>>,
    handle: JoinHandle<()>,
    _marker: PhantomData<F>,
}

impl<K, V, F> TableWriter<K, V, F>
where
    K: Key + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V> + Send + Clone + 'static,
{
    fn step<T: WriteTableLike<K, V>>(table: &mut T, cmd: WriterCommand<K, V>) -> Result<Control, AppError> {
        match cmd {
            WriterCommand::Insert(k, v) => {
                table.insert_kv(k, v)?;
                Ok(Control::Continue)
            }
            WriterCommand::Remove(k, ack) => {
                let r = table.delete_kv(k)?;
                let _ = ack.send(Ok(r));
                Ok(Control::Continue)
            }
            WriterCommand::AnyForIndex(values, ack) => {
                let mut out = Vec::with_capacity(values.len());
                for v in values {
                    out.push(table.get_any_for_index(v)?);
                }
                let _ = ack.send(Ok(out));
                Ok(Control::Continue)
            }
            WriterCommand::AnyForIndexTagged(pairs, ack) => {
                let mut out = Vec::with_capacity(pairs.len());
                for (pos, v) in pairs {
                    let any = table.get_any_for_index(v)?;
                    out.push((pos, any));
                }
                let _ = ack.send(Ok(out));
                Ok(Control::Continue)
            }
            WriterCommand::Range(from, until, ack) => {
                let r = table.range(from..until)?;
                let _ = ack.send(Ok(r));
                Ok(Control::Continue)
            }
            WriterCommand::Flush(ack) => Ok(Control::Flush(ack)),
            WriterCommand::Shutdown(ack) => Ok(Control::Shutdown(ack)),
            WriterCommand::Begin(_) => unreachable!("Begin handled outside"),
        }
    }

    fn drain_batch<T: WriteTableLike<K, V>>(table: &mut T, rx: &Receiver<WriterCommand<K, V>>) -> Result<Control, AppError> {
        // 1) one blocking recv to ensure progress
        let mut ctrl = Self::step(table, rx.recv()?)?;
        if !matches!(ctrl, Control::Continue) {
            return Ok(ctrl);
        }

        // 2) opportunistically drain the channel without blocking
        for cmd in rx.try_iter() {
            ctrl = Self::step(table, cmd)?;
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
            let name = factory.name();
            'outer: loop {
                // wait until someone asks us to begin a write tx
                let cmd = match receiver.recv() {
                    Ok(c) => c,
                    Err(e) => { error!("writer terminated: {}", e.to_string()); break; }
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
                        // 0) open a new write tx
                        let tx = match db_arc.begin_write() {
                            Ok(tx) => tx,
                            Err(e) => { let _ = ack.send(Err(AppError::from(e))); continue 'outer; }
                        };
                        // 1) drop the strong Arc immediately; owner keeps DB alive
                        drop(db_arc);
                        // 2) open typed table bound to &tx
                        let mut table = match factory.open(&tx, &mut cache) {
                            Ok(t) => { let _ = ack.send(Ok(())); t },
                            Err(e) => { let _ = ack.send(Err(e)); continue 'outer; }
                        };
                        // 3) process commands until a Flush arrives
                        let mut flush_ack: Option<Sender<Result<TaskResult, AppError>>> = None;
                        let mut write_error: Option<Result<(), AppError>> = None;
                        let t0 = Instant::now();

                        'in_tx: loop {
                            match Self::drain_batch(&mut table, &receiver) {
                                Ok(Control::Continue) => continue,
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
                                    let _ = tx.commit().map_err(AppError::from);
                                    let took = t0.elapsed().as_millis();
                                    ack.send(Ok(TaskResult::new(&name, took)))
                                },
                            };
                        } else {
                           error!("Transaction ended without Flush or Shutdown, it can never happen");
                        }
                        // go back to idle and wait for next Begin
                    }

                    WriterCommand::Shutdown(ack) => {
                        // no active tx at this point; stop thread
                        let _ = ack.send(Ok(()));
                        break 'outer;
                    }

                    other => {
                        error!("received {:?} before Begin; ignoring", std::mem::discriminant(&other));
                    }
                }
            }
        });
        Ok(Self { topic, handle, _marker: PhantomData })
    }

    pub fn fast_send(&self,  msg: WriterCommand<K, V>) -> Result<(), AppError> {
        match self.topic.try_send(msg) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(v)) => self.topic.send(v).map_err(|e| AppError::Custom(e.to_string())),
            Err(e) => Err(AppError::Custom(e.to_string())),
        }
    }

    // ---- new API to (re)begin a transaction ----
    pub fn begin(&self) -> Result<(), AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<(), AppError>>(1);
        self.fast_send(WriterCommand::Begin(ack_tx))?;
        ack_rx.recv()?
    }

    // your existing ops now must be called after begin()
    pub fn insert_kv(&self, key: K, value: V) -> Result<(), AppError> {
        self.fast_send(WriterCommand::Insert(key, value))
    }

    pub fn get_any_for_index_tagged(&self, pairs: Vec<(usize, V)>) -> Result<Vec<(usize, Option<ValueBuf<K>>)>, AppError> {
        let (ack_tx, ack_rx) = bounded(1);
        self.fast_send(WriterCommand::AnyForIndexTagged(pairs, ack_tx))?;
        ack_rx.recv()?
    }

    pub fn get_any_for_index(&self, values: Vec<V>) -> Result<Vec<Option<ValueBuf<K>>>, AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<Vec<Option<ValueBuf<K>>>, AppError>>(1);
        self.fast_send(WriterCommand::AnyForIndex(values, ack_tx))?;
        ack_rx.recv()?
    }

    pub fn range(&self, from: K, until: K) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>>(1);
        self.fast_send(WriterCommand::Range(from, until, ack_tx))?;
        ack_rx.recv()?
    }

    pub fn delete_kv(&self, key: K) -> Result<bool, AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<bool, AppError>>(1);
        self.fast_send(WriterCommand::Remove(key, ack_tx))?;
        ack_rx.recv()?
    }

    // commit current tx but KEEP worker alive (idle); you can call begin() again
    pub fn flush(&self) -> Result<TaskResult, AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<TaskResult, AppError>>(1);
        self.fast_send(WriterCommand::Flush(ack_tx))?;
        ack_rx.recv()?
    }

    pub fn flush_async(&self) -> Result<Vec<FlushFuture>, AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<TaskResult, AppError>>(1);
        self.fast_send(WriterCommand::Flush(ack_tx))?;
        Ok(vec![FlushFuture { ack_rx }])
    }
    // optional: graceful shutdown when you’re done with the writer forever
    pub fn shutdown(self) -> Result<(), AppError> {
        let (ack_tx, ack_rx) = bounded::<Result<(), AppError>>(1);
        self.fast_send(WriterCommand::Shutdown(ack_tx))?;
        ack_rx.recv()??;
        self.handle.join().map_err(|_| AppError::Custom("Write table join failed".to_string()))?;
        Ok(())
    }
}
