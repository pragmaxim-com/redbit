use crate::{error, AppError};
use redb::{AccessGuard, Database, Key, Value, WriteTransaction};
use std::borrow::Borrow;
use std::marker::PhantomData;
use std::ops::RangeBounds;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use crossbeam::channel::{bounded, unbounded, Receiver, Sender, TryRecvError, TrySendError};

pub struct FlushFuture {
    ack_rx: Receiver<redb::Result<(), AppError>>,
    handle: JoinHandle<()>,
}

impl FlushFuture {
    pub fn wait(self) -> redb::Result<(), AppError> {
        let res = self.ack_rx.recv()??;
        self.handle.join().map_err(|_| AppError::Custom("Write table join failed".to_string()))?;
        Ok(res)
    }
}

pub struct ValueBuf<V: Value> {
    buf: Vec<u8>,
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
    fn get_head_by_index<'v>(&self, value: impl Borrow<V::SelfType<'v>>) -> redb::Result<Option<ValueBuf<K>>>;
    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, range: impl RangeBounds<KR> + 'a) -> redb::Result<Vec<(ValueBuf<K>, ValueBuf<V>)>>;

    fn key_buf(g: AccessGuard<'_, K>) -> ValueBuf<K> {
        ValueBuf::<K>::new(K::as_bytes(&g.value()).as_ref().to_vec())
    }
    fn value_buf(g: AccessGuard<'_, V>) -> ValueBuf<V> {
        ValueBuf::<V>::new(V::as_bytes(&g.value()).as_ref().to_vec())
    }

}

pub trait TableFactory<K: Key + 'static, V: Key + 'static> {
    type Table<'txn>: WriteTableLike<'txn, K, V>;

    fn open<'txn>(&self, tx: &'txn WriteTransaction) -> redb::Result<Self::Table<'txn>, AppError>;
}

pub enum WriterCommand<K: Key + Send + 'static, V: Key + Send + 'static> {
    Insert(K, V),
    Remove(K, Sender<redb::Result<bool, AppError>>),
    HeadByIndex(Vec<V>, Sender<redb::Result<Vec<Option<ValueBuf<K>>>, AppError>>),
    Range(K, K, Sender<redb::Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>>),
    Flush(Sender<redb::Result<(), AppError>>),
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
    F::Table<'static>: Send,
{
    fn handle_cmd<'txn, T>(table: &mut T, cmd: WriterCommand<K, V>) -> Result<Option<Sender<redb::Result<(), AppError>>>, AppError>
    where
        T: WriteTableLike<'txn, K, V>,
    {
        match cmd {
            WriterCommand::Insert(k, v) => {
                table.insert_kv(k, v)?;
                Ok(None)
            }
            WriterCommand::Remove(k, ack) => {
                let res = table.delete_kv(k)?;
                let _ = ack.send(Ok(res));
                Ok(None)
            }
            WriterCommand::HeadByIndex(values, ack) => {
                let mut result = Vec::with_capacity(values.len());
                for value in values {
                    result.push(table.get_head_by_index(value)?);
                }
                let _ = ack.send(Ok(result));
                Ok(None)
            }
            WriterCommand::Range(from, until, ack) => {
                let result = table.range(from..until)?;
                let _ = ack.send(Ok(result));
                Ok(None)
            }
            WriterCommand::Flush(ack) => {
                Ok(Some(ack))
            }
        }
    }

    pub fn new(dict_db: Arc<Database>, factory: F) -> redb::Result<Self, AppError> {
        let (topic, receiver): (Sender<WriterCommand<K, V>>, Receiver<WriterCommand<K, V>>) = unbounded();
        let (ready_tx, ready_rx) = bounded::<redb::Result<(), AppError>>(1);

        let handle = thread::spawn(move || {
            let tx = match dict_db.begin_write() {
                Ok(tx) => tx,
                Err(e) => {
                    let _ = ready_tx.send(Err(AppError::from(e)));
                    return;
                }
            };
            let mut flush_ack: Option<Sender<redb::Result<(), AppError>>> = None;
            let mut write_error: Option<Result<(), AppError>> = None;

            match factory.open(&tx) {
                Ok(mut table) => {
                    let _ = ready_tx.send(Ok(()));
                    'outer: loop {
                        let cmd = match receiver.recv() {
                            Ok(c) => c,
                            Err(recv_error) => {
                                error!("Writer thread receiver error: {:?}", recv_error);
                                break
                            },
                        };

                        match Self::handle_cmd(&mut table, cmd) {
                            Err(err) => if write_error.is_none() {
                                write_error = Some(Err(err));
                            }
                            Ok(Some(flush)) => {
                                flush_ack = Some(flush);
                                break
                            },
                            Ok(None) => { /* continue */ }
                        }

                        // opportunistically drain more without sleeping (reduces per-item sync)
                        loop {
                            match receiver.try_recv() {
                                Ok(c) => {
                                    match Self::handle_cmd(&mut table, c) {
                                        Err(err) => if write_error.is_none() {
                                            write_error = Some(Err(err));
                                        }
                                        Ok(Some(flush)) => {
                                            flush_ack = Some(flush);
                                            break 'outer
                                        },
                                        Ok(None) => { /* continue */ }
                                    }
                                }
                                Err(TryRecvError::Empty) => break,
                                Err(TryRecvError::Disconnected) => break,
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
            if let Some(ack) = flush_ack {
                let _ = match write_error {
                    Some(Err(e)) => ack.send(Err(e)),
                    _ => ack.send(tx.commit().map_err(AppError::from))
                };
            }
        });

        // wait until the worker tells us it opened the table successfully
        ready_rx.recv()??;
        Ok(Self { topic, handle, _marker: PhantomData  })
    }

    #[inline]
    fn fast_send(tx: &Sender<WriterCommand<K, V>>, msg: WriterCommand<K, V>) -> Result<(), AppError> {
        match tx.try_send(msg) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(v)) => { tx.send(v).map_err(|err| AppError::Custom(err.to_string())) }
            Err(err) => Err(AppError::Custom(err.to_string())),
        }
    }

    pub fn insert_kv(&self, key: K, value: V) -> Result<(), AppError> {
        Self::fast_send(&self.topic, WriterCommand::Insert(key, value))
    }

    // hack, we need to read from the writer thread
    pub fn get_head_for_index(&self, values: Vec<V>) -> redb::Result<Vec<Option<ValueBuf<K>>>, AppError> {
        let (ack_tx, ack_rx) = bounded::<redb::Result<Vec<Option<ValueBuf<K>>>, AppError>>(1);
        Self::fast_send(&self.topic, WriterCommand::HeadByIndex(values, ack_tx))?;
        let result = ack_rx.recv()??;
        Ok(result)
    }

    // hack, we need to read from the writer thread
    pub fn range(&self, from: K, until: K) -> redb::Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError> {
        let (ack_tx, ack_rx) = bounded::<redb::Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>>(1);
        Self::fast_send(&self.topic, WriterCommand::Range(from, until, ack_tx))?;
        let result = ack_rx.recv()??;
        Ok(result)
    }

    pub fn delete_kv(&self, key: K) -> redb::Result<bool, AppError> {
        let (ack_tx, ack_rx) = bounded::<redb::Result<bool, AppError>>(1);
        Self::fast_send(&self.topic, WriterCommand::Remove(key, ack_tx))?;
        let result = ack_rx.recv()?;
        result
    }

    pub fn flush(self) -> redb::Result<(), AppError> {
        let (ack_tx, ack_rx) = bounded::<redb::Result<(), AppError>>(1);
        Self::fast_send(&self.topic, WriterCommand::Flush(ack_tx))?;
        FlushFuture { ack_rx, handle: self.handle }.wait()
    }

    pub fn flush_async(self) -> redb::Result<FlushFuture, AppError> {
        let (ack_tx, ack_rx) = bounded::<redb::Result<(), AppError>>(1);
        Self::fast_send(&self.topic, WriterCommand::Flush(ack_tx))?;
        Ok(FlushFuture { ack_rx, handle: self.handle })
    }
}
