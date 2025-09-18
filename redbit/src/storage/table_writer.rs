use crate::{error, AppError};
use crossbeam::channel::{bounded, unbounded, Receiver, Sender, TrySendError};
use redb::{AccessGuard, Database, Key, Value, WriteTransaction};
use std::borrow::Borrow;
use std::marker::PhantomData;
use std::ops::RangeBounds;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;

pub struct FlushFuture {
    ack_rx: Receiver<redb::Result<(), AppError>>,
}

impl FlushFuture {
    pub fn wait(self) -> redb::Result<(), AppError> {
        self.ack_rx.recv()?
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

pub trait WriteTableLike<'txn, K: Key + 'static, V: Key + 'static> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> redb::Result<(), AppError>;
    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> redb::Result<bool, AppError>;
    fn get_head_by_index<'v>(&mut self, value: impl Borrow<V::SelfType<'v>>) -> redb::Result<Option<ValueBuf<K>>>;
    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, range: impl RangeBounds<KR> + 'a) -> redb::Result<Vec<(ValueBuf<K>, ValueBuf<V>)>>;

    fn key_buf(g: AccessGuard<'_, K>) -> ValueBuf<K> {
        ValueBuf::<K>::new(K::as_bytes(&g.value()).as_ref().to_vec())
    }
    fn value_buf(g: AccessGuard<'_, V>) -> ValueBuf<V> {
        ValueBuf::<V>::new(V::as_bytes(&g.value()).as_ref().to_vec())
    }
}

pub trait TableFactory<K: Key + 'static, V: Key + 'static> {
    type CacheCtx;
    type Table<'txn, 'c>: WriteTableLike<'txn, K, V>;

    fn new_cache(&self) -> Self::CacheCtx;
    fn open<'txn, 'c>(&self, tx: &'txn WriteTransaction, cache: &'c mut Self::CacheCtx) -> Result<Self::Table<'txn, 'c>, AppError>;
}

pub enum WriterCommand<K: Key + Send + 'static, V: Key + Send + 'static> {
    Begin(Sender<redb::Result<(), AppError>>),              // start new WriteTransaction + open table
    Insert(K, V),
    Remove(K, Sender<redb::Result<bool, AppError>>),
    HeadByIndex(Vec<V>, Sender<redb::Result<Vec<Option<ValueBuf<K>>>, AppError>>),
    Range(K, K, Sender<redb::Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>>),
    Flush(Sender<redb::Result<(), AppError>>),              // commit current tx, stay alive (idle)
    Shutdown(Sender<redb::Result<(), AppError>>),           // graceful stop (no commit)
}

enum Control {
    Continue,
    Flush(Sender<redb::Result<(), AppError>>),
    Shutdown(Sender<redb::Result<(), AppError>>),
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
    F: TableFactory<K, V> + Send + 'static,
    F::Table<'static, 'static>: Send,
{
    fn step<'txn, T>(table: &mut T, cmd: WriterCommand<K, V>) -> Result<Control, AppError>
    where
        T: WriteTableLike<'txn, K, V>,
    {
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
            WriterCommand::HeadByIndex(values, ack) => {
                let mut out = Vec::with_capacity(values.len());
                for v in values {
                    out.push(table.get_head_by_index(v)?);
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

    fn drain_batch<'txn, T>(table: &mut T, rx: &Receiver<WriterCommand<K, V>>) -> Result<Control, AppError>
    where
        T: WriteTableLike<'txn, K, V>,
    {
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

    pub fn new(db: Arc<Database>, factory: F) -> redb::Result<Self, AppError> {
        let (topic, receiver): (Sender<WriterCommand<K, V>>, Receiver<WriterCommand<K, V>>) = unbounded();
        let handle = thread::spawn(move || {
            let mut cache = factory.new_cache();
            'outer: loop {
                // wait until someone asks us to begin a write tx
                let cmd = match receiver.recv() {
                    Ok(c) => c,
                    Err(e) => { error!("writer terminated: {}", e.to_string()); break; }
                };

                match cmd {
                    WriterCommand::Begin(ack) => {
                        // 1) open a new write tx
                        let tx = match db.begin_write() {
                            Ok(tx) => tx,
                            Err(e) => { let _ = ack.send(Err(AppError::from(e))); continue 'outer; }
                        };
                        // 2) open typed table bound to &tx
                        let mut table = match factory.open(&tx, &mut cache) {
                            Ok(t) => { let _ = ack.send(Ok(())); t },
                            Err(e) => { let _ = ack.send(Err(e)); continue 'outer; }
                        };

                        // 3) process commands until a Flush arrives
                        let mut flush_ack: Option<Sender<redb::Result<(), AppError>>> = None;
                        let mut write_error: Option<Result<(), AppError>> = None;

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
                                _ => ack.send(tx.commit().map_err(AppError::from)),
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

    #[inline]
    fn fast_send(tx: &Sender<WriterCommand<K, V>>, msg: WriterCommand<K, V>) -> Result<(), AppError> {
        match tx.try_send(msg) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(v)) => tx.send(v).map_err(|e| AppError::Custom(e.to_string())),
            Err(e) => Err(AppError::Custom(e.to_string())),
        }
    }

    // ---- new API to (re)begin a transaction ----
    pub fn begin(&self) -> redb::Result<(), AppError> {
        let (ack_tx, ack_rx) = bounded::<redb::Result<(), AppError>>(1);
        Self::fast_send(&self.topic, WriterCommand::Begin(ack_tx))?;
        ack_rx.recv()?
    }

    // your existing ops now must be called after begin()
    pub fn insert_kv(&self, key: K, value: V) -> Result<(), AppError> {
        Self::fast_send(&self.topic, WriterCommand::Insert(key, value))
    }

    pub fn get_head_for_index(&self, values: Vec<V>) -> redb::Result<Vec<Option<ValueBuf<K>>>, AppError> {
        let (ack_tx, ack_rx) = bounded::<redb::Result<Vec<Option<ValueBuf<K>>>, AppError>>(1);
        Self::fast_send(&self.topic, WriterCommand::HeadByIndex(values, ack_tx))?;
        ack_rx.recv()?
    }

    pub fn range(&self, from: K, until: K) -> redb::Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError> {
        let (ack_tx, ack_rx) = bounded::<redb::Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>>(1);
        Self::fast_send(&self.topic, WriterCommand::Range(from, until, ack_tx))?;
        ack_rx.recv()?
    }

    pub fn delete_kv(&self, key: K) -> redb::Result<bool, AppError> {
        let (ack_tx, ack_rx) = bounded::<redb::Result<bool, AppError>>(1);
        Self::fast_send(&self.topic, WriterCommand::Remove(key, ack_tx))?;
        ack_rx.recv()?
    }

    // commit current tx but KEEP worker alive (idle); you can call begin() again
    pub fn flush(&self) -> redb::Result<(), AppError> {
        let (ack_tx, ack_rx) = bounded::<redb::Result<(), AppError>>(1);
        Self::fast_send(&self.topic, WriterCommand::Flush(ack_tx))?;
        ack_rx.recv()?
    }

    pub fn flush_async(&self) -> redb::Result<FlushFuture, AppError> {
        let (ack_tx, ack_rx) = bounded::<redb::Result<(), AppError>>(1);
        Self::fast_send(&self.topic, WriterCommand::Flush(ack_tx))?;
        Ok(FlushFuture { ack_rx })
    }
    // optional: graceful shutdown when you’re done with the writer forever
    pub fn shutdown(self) -> redb::Result<(), AppError> {
        let (ack_tx, ack_rx) = bounded::<redb::Result<(), AppError>>(1);
        Self::fast_send(&self.topic, WriterCommand::Shutdown(ack_tx))?;
        let res = ack_rx.recv()??;
        self.handle.join().map_err(|_| AppError::Custom("Write table join failed".to_string()))?;
        Ok(res)
    }
}
