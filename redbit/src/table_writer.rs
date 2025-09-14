use crate::AppError;
use redb::{AccessGuard, Database, Key, MultimapValue, Value, WriteTransaction};
use std::borrow::Borrow;
use std::marker::PhantomData;
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;

pub struct ValueBuf<V: Value> {
    buf: Vec<u8>,
    _pd: PhantomData<V>,
}

impl<V: Value> ValueBuf<V> {
    pub fn new(buf: Vec<u8>) -> Self { Self { buf, _pd: PhantomData } }
    pub fn as_value(&self) -> V::SelfType<'_> { V::from_bytes(&self.buf) }
    pub fn as_bytes(&self) -> &[u8] { &self.buf }
}

pub trait WriteTableLike<K: Key + 'static, V: Key + 'static> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> redb::Result<(), AppError>;
    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> redb::Result<bool, AppError>;
    fn get_by_index<'v>(&self, value: impl Borrow<V::SelfType<'v>>) -> redb::Result<MultimapValue<'_, K>>;
}

pub trait TableFactory<K: Key + 'static, V: Key + 'static> {
    type Table<'txn>: WriteTableLike<K, V> + 'txn;

    fn open<'txn>(&self, tx: &'txn WriteTransaction) -> redb::Result<Self::Table<'txn>, AppError>;
}

pub enum WriterCommand<K: Key + Send + 'static, V: Key + Send + 'static> {
    Insert(K, V),
    Remove(K, Sender<redb::Result<bool, AppError>>),
    GetKeysForValues(Vec<V>, Sender<redb::Result<Vec<Option<ValueBuf<K>>>, AppError>>),
    Flush,
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
    pub fn guard_to_value_buf(g: AccessGuard<'_, K>) -> ValueBuf<K> {
        ValueBuf::<K>::new(K::as_bytes(&g.value()).as_ref().to_vec())
    }

    pub fn new(dict_db: Arc<Database>, factory: F) -> redb::Result<Self, AppError> {
        let (topic, receiver) = channel::<WriterCommand<K, V>>();
        let (ready_tx, ready_rx) = channel::<redb::Result<(), AppError>>();

        let handle = thread::spawn(move || {
            let tx = match dict_db.begin_write() {
                Ok(tx) => tx,
                Err(e) => {
                    let _ = ready_tx.send(Err(AppError::from(e)));
                    return;
                }
            };

            match factory.open(&tx) {
                Ok(mut table) => {
                    let _ = ready_tx.send(Ok(()));
                    loop {
                        match receiver.recv() {
                            Ok(WriterCommand::Insert(k, v)) => {
                                let _ = table.insert_kv(k, v);
                            }
                            Ok(WriterCommand::Remove(k, ack)) => {
                                let result = table.delete_kv(k);
                                let _ = ack.send(result);
                            }
                            Ok(WriterCommand::GetKeysForValues(values, ack)) => {
                                let mut result: Vec<Option<ValueBuf<K>>> = Vec::with_capacity(values.len());
                                for value in values {
                                    let mm = table.get_by_index(value);
                                    if let Some(guard) = mm.unwrap().next() {
                                        let vbuf = Self::guard_to_value_buf(guard.unwrap());
                                        result.push(Some(vbuf));
                                    } else {
                                        result.push(None);
                                    }
                                }
                                let _ = ack.send(Ok(result));
                            }
                            Ok(WriterCommand::Flush) | Err(_) => break,
                        }
                    }
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
            tx.commit().expect("commit worker tx");
        });

        // wait until the worker tells us it opened the table successfully
        ready_rx.recv()??;
        Ok(Self { topic, handle, _marker: PhantomData  })
    }

    pub fn insert_kv(&self, key: K, value: V) {
        self.topic.send(WriterCommand::Insert(key, value)).unwrap();
    }

    pub fn get_keys_for_values(&self, values: Vec<V>) -> redb::Result<Vec<Option<ValueBuf<K>>>, AppError> {
        let (ack_tx, ack_rx) = channel::<redb::Result<Vec<Option<ValueBuf<K>>>, AppError>>();
        self.topic.send(WriterCommand::GetKeysForValues(values, ack_tx)).unwrap();
        let result = ack_rx.recv()??;
        Ok(result)
    }

    pub fn delete_kv(&self, key: K) -> redb::Result<bool, AppError> {
        let (ack_tx, ack_rx) = channel::<redb::Result<bool, AppError>>();
        self.topic.send(WriterCommand::Remove(key, ack_tx)).unwrap();
        let result = ack_rx.recv()?;
        result
    }

    pub fn flush(self) -> redb::Result<(), AppError> {
        self.topic.send(WriterCommand::Flush).unwrap();
        self.handle.join().map_err(|_| AppError::Custom("Write table join failed".to_string()))
    }
}

