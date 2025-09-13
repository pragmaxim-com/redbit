use crate::AppError;
use redb::{Database, Key, WriteTransaction};
use std::borrow::Borrow;
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;

pub trait WriteTableLike<K: Key + 'static, V: Key + 'static> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> redb::Result<(), AppError>;
    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> redb::Result<bool, AppError>;
}

pub trait TableFactory<K: Key + 'static, V: Key + 'static> {
    type Table<'txn>: WriteTableLike<K, V> + 'txn;

    fn open<'txn>(&self, tx: &'txn WriteTransaction) -> redb::Result<Self::Table<'txn>, AppError>;
}

pub enum IndexCommand<K: Key + Send + 'static, V: Key + Send + 'static> {
    Insert(K, V),
    Remove(K, Sender<redb::Result<bool, AppError>>), // ✅ send back signal when done
    Flush,
}

pub struct TableWriter<K: Key + Send + 'static, V: Key + Send + 'static, F> {
    topic: Sender<IndexCommand<K, V>>,
    handle: JoinHandle<()>,
    _marker: std::marker::PhantomData<F>,
}

impl<K, V, F> TableWriter<K, V, F>
where
    K: Key + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: TableFactory<K, V> + Send + 'static,
    F::Table<'static>: Send,
{
    pub fn new(dict_db: Arc<Database>, factory: F) -> redb::Result<Self, AppError> {
        let (topic, receiver) = channel::<IndexCommand<K, V>>();
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
                            Ok(IndexCommand::Insert(k, v)) => {
                                let _ = table.insert_kv(k, v);
                            }
                            Ok(IndexCommand::Remove(k, ack)) => {
                                let result = table.delete_kv(k);
                                let _ = ack.send(result);
                            }
                            Ok(IndexCommand::Flush) | Err(_) => break,
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
        Ok(Self { topic, handle, _marker: std::marker::PhantomData  })
    }

    pub fn insert_kv(&self, key: K, value: V) {
        self.topic.send(IndexCommand::Insert(key, value)).unwrap();
    }

    pub fn delete_kv(&self, key: K) -> redb::Result<bool, AppError> {
        let (ack_tx, ack_rx) = channel::<redb::Result<bool, AppError>>();
        self.topic.send(IndexCommand::Remove(key, ack_tx)).unwrap();
        let result = ack_rx.recv()?;
        result
    }

    pub fn flush(self) -> redb::Result<(), AppError> {
        self.topic.send(IndexCommand::Flush).unwrap();
        self.handle.join().map_err(|_| AppError::Custom("Write table join failed".to_string()))
    }
}

