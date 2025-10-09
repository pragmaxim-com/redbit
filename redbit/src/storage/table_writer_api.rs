use crate::storage::async_boundary::{CopyOwnedValue, ValueBuf, ValueOwned};
use crate::storage::router::Router;
use crate::AppError;
use crossbeam::channel::{Receiver, Sender};
use redb::{AccessGuard, Key, Value, WriteTransaction};
use std::borrow::Borrow;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ops::RangeBounds;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::fmt;

#[derive(Clone, Debug)]
pub struct TaskResult {
    pub name: String,
    pub write_took: u128,
    pub commit_took: u128,
}

impl fmt::Display for TaskResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} : {} ms", self.name, self.write_took)
    }
}

impl TaskResult {
    pub fn new(name: &str, write_took: u128, commit_took: u128) -> Self {
        Self { name: name.to_string(), write_took, commit_took }
    }
    pub fn master(write_took: u128) -> Self {
        Self { name: "MASTER".to_string(), write_took, commit_took: 0}
    }
}

pub struct StartFuture(pub Receiver<Result<(), AppError>>);
impl StartFuture {
    pub fn wait(self) -> Result<(), AppError> {
        self.0.recv()?
    }
}

pub struct StopFuture {
    pub(crate) ack: Receiver<Result<(), AppError>>,
    pub(crate) handle: JoinHandle<()>,
}
impl StopFuture {
    pub fn wait(self) -> Result<(), AppError> {
        self.ack.recv()??;
        self.handle.join().map_err(|_| AppError::Custom("Write table join failed".to_string()))?;
        Ok(())
    }
}

pub enum FlushFuture {
    /// Flush already sent; just wait for the ack
    Eager(Receiver<Result<TaskResult, AppError>>),

    /// Flush will be sent when `wait()` is called
    Lazy(Box<dyn FnOnce() -> Result<Receiver<Result<TaskResult, AppError>>, AppError> + Send>),
}

impl FlushFuture {
    pub fn wait(self) -> Result<TaskResult, AppError> {
        match self {
            FlushFuture::Eager(rx) => rx.recv()?,
            FlushFuture::Lazy(fire) => {
                let rx = fire()?;
                rx.recv()?
            }
        }
    }

    pub fn eager(rx: Receiver<Result<TaskResult, AppError>>) -> Self {
        FlushFuture::Eager(rx)
    }

    pub fn lazy<F>(f: F) -> Self
    where
        F: FnOnce() -> Result<Receiver<Result<TaskResult, AppError>>, AppError> + Send + 'static,
    {
        FlushFuture::Lazy(Box::new(f))
    }
}

impl FlushFuture {

    pub fn dedup_tasks_keep_slowest(futs: Vec<FlushFuture>) -> Result<HashMap<String, TaskResult>, AppError> {
        let mut by_name: HashMap<String, TaskResult> = HashMap::with_capacity(futs.len());
        for f in futs {
            let res = f.wait()?;
            match by_name.entry(res.name.clone()) {
                Entry::Vacant(e) => { e.insert(res); }
                Entry::Occupied(mut e) => {
                    if res.write_took > e.get().write_took {
                        e.insert(res); // keep the slowest per name
                    }
                }
            }
        }
        Ok(by_name)
    }
}

pub trait WriteTableLike<K: CopyOwnedValue + 'static, V: Key + 'static> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<(), AppError>;
    fn insert_many_kvs<'k, 'v, KR: Borrow<K::SelfType<'k>>, VR: Borrow<V::SelfType<'v>>>(&mut self,  pairs: Vec<(KR, VR)>, sort_by_key: bool) -> Result<(), AppError>;
    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> Result<bool, AppError>;
    fn get_any_for_index<'v>(&mut self, value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueOwned<K>>, AppError>;
    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, range: impl RangeBounds<KR> + 'a) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>;

    #[inline]
    fn key_buf(g: AccessGuard<'_, K>) -> ValueBuf<K> {
        ValueBuf::<K>::new(K::as_bytes(&g.value()).as_ref().to_vec())
    }
    #[inline]
    fn value_buf(g: AccessGuard<'_, V>) -> ValueBuf<V> {
        ValueBuf::<V>::new(V::as_bytes(&g.value()).as_ref().to_vec())
    }
    #[inline]
    fn owned_key_from_bytes(bytes: &[u8]) -> ValueOwned<K> {
        let k_view: K::SelfType<'_> = <K as Value>::from_bytes(bytes);
        ValueOwned::<K>::from_value(k_view)
    }
    #[inline]
    fn owned_key_from_guard(g: AccessGuard<'_, K>) -> ValueOwned<K> {
        ValueOwned::<K>::from_guard(g)
    }

    #[inline]
    fn unit_from_key<'k>(k: &K::SelfType<'k>) -> K::Unit
    where
        K: 'k,
    {
        K::to_unit_ref(k)
    }

    #[inline]
    fn owned_from_unit(u: K::Unit) -> ValueOwned<K> {
        ValueOwned::<K>::from_unit(u)
    }
}

pub trait TableFactory<K: CopyOwnedValue + 'static, V: Key + 'static> {
    type CacheCtx;
    type Table<'txn, 'c>: WriteTableLike<K, V>;
    fn name(&self) -> String;
    fn new_cache(&self) -> Self::CacheCtx;
    fn open<'txn, 'c>(&self, tx: &'txn WriteTransaction, cache: &'c mut Self::CacheCtx) -> Result<Self::Table<'txn, 'c>, AppError>;
}

pub enum WriterCommand<K: CopyOwnedValue + Send + 'static, V: Key + Send + 'static> {
    Begin(Sender<Result<(), AppError>>),              // start new WriteTransaction + open table
    InsertMany(Vec<(K, V)>),
    Remove(K, Sender<Result<bool, AppError>>),
    QueryAndWrite { values: Vec<V>, sink: Arc<dyn Fn(Vec<(usize, Option<ValueOwned<K>>)>) -> Result<(), AppError> + Send + Sync + 'static> },
    QueryAndWriteBucket { values: Vec<(usize, V)>, sink: Arc<dyn Fn(Vec<(usize, Option<ValueOwned<K>>)>) -> Result<(), AppError> + Send + Sync + 'static> },
    Range(K, K, Sender<Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>>),
    Flush(Sender<Result<TaskResult, AppError>>),              // commit current tx, stay alive (idle)
    Shutdown(Sender<Result<(), AppError>>),           // graceful stop (no commit)
}

pub enum Control {
    Continue,
    Flush(Sender<Result<TaskResult, AppError>>),
    Shutdown(Sender<Result<(), AppError>>),
}

pub trait WriterLike<K: CopyOwnedValue, V: Value> {
    fn router(&self) -> Arc<dyn Router<K, V>>;
    fn begin(&self) -> Result<(), AppError>;
    fn begin_async(&self) -> Result<Vec<StartFuture>, AppError>;
    fn insert_kv(&self, key: K, value: V) -> Result<(), AppError>;
    fn flush(&self) -> Result<TaskResult, AppError>;
    fn flush_async(&self) -> Result<Vec<FlushFuture>, AppError>;
    fn flush_deferred(&self) -> Vec<FlushFuture>;
    fn shutdown(self) -> Result<(), AppError> where Self: Sized;
    fn shutdown_async(self) -> Result<Vec<StopFuture>, AppError> where Self: Sized;
}
