use crate::storage::async_boundary::{CopyOwnedValue, ValueBuf, ValueOwned};
use crate::storage::context::{ToReadField, ToWriteField};
use crate::storage::router::{Router, ShardedRouter};
use crate::{AppError, KeyPartitioner, Partitioning, ShardedReadOnlyDictTable, ShardedReadOnlyIndexTable, ShardedReadOnlyPlainTable, ShardedTableWriter, Storage, TableInfo, TxFSM, ValuePartitioner};
use crossbeam::channel::{bounded, Receiver, Sender};
use redb::{AccessGuard, Database, Durability, Key, MultimapValue, Value, WriteTransaction};
use std::borrow::Borrow;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::ops::RangeBounds;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;

#[derive(Clone, Debug)]
pub struct TaskStats {
    pub collect_took: u128,
    pub sort_took: u128,
    pub write_took: u128,
    pub flush_took: u128,
}
impl TaskStats {
    pub fn new(collect_took: u128, sort_took: u128, write_took: u128, flush_took: u128) -> Self {
        Self { collect_took, sort_took, write_took, flush_took }
    }
    pub fn sum(&self) -> u128 {
        self.collect_took + self.sort_took + self.write_took + self.flush_took
    }
}

#[derive(Clone, Debug)]
pub struct TaskResult {
    pub name: String,
    pub stats: TaskStats
}

impl fmt::Display for TaskResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} : c/s/w/c : {}/{}/{}/{} ms", self.name, self.stats.collect_took, self.stats.sort_took, self.stats.write_took, self.stats.flush_took)
    }
}

impl TaskResult {
    pub fn new(name: &str, stats: TaskStats) -> Self {
        Self { name: name.to_string(), stats }
    }
    pub fn master(master_took: u128) -> Self {
        Self { name: "MASTER".to_string(), stats: TaskStats::new(master_took, 0, 0, 0) }
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

    /// Helper: produce the shared "fire" closure that sends WriterCommand::Flush
    fn flush<K: CopyOwnedValue + Send + 'static, V: Key + Send + 'static>(
        tx: Sender<WriterCommand<K, V>>,
    ) -> Box<dyn FnOnce() -> Result<Receiver<Result<TaskResult, AppError>>, AppError> + Send> {
        Box::new(move || {
            let (ack_tx, ack_rx) = bounded::<Result<TaskResult, AppError>>(1);
            match tx.send(WriterCommand::Flush(ack_tx)) {
                Ok(()) => Ok(ack_rx),
                Err(e) => Err(AppError::Custom(e.to_string())),
            }
        })
    }

    pub fn lazy<K: CopyOwnedValue + Send + 'static, V: Key + Send + 'static>(tx: Sender<WriterCommand<K, V>>) -> Self {
        FlushFuture::Lazy(Self::flush(tx))
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
                    if res.stats.sum() > e.get().stats.sum() {
                        e.insert(res); // keep the slowest per name
                    }
                }
            }
        }
        Ok(by_name)
    }
}

pub trait ReadTableLike<K: Key + 'static, V: Key + 'static> {
    fn index_keys<'v>(&self, val: impl Borrow<V::SelfType<'v>>) -> Result<MultimapValue<'static, K>, AppError>;
    fn dict_keys<'v>(&self, val: impl Borrow<V::SelfType<'v>>) -> redb::Result<Option<MultimapValue<'static, K>>, AppError>;
    fn get_value<'k>(&self, key: impl Borrow<K::SelfType<'k>>) -> Result<Option<AccessGuard<'_, V>>, AppError>;
    fn iter_keys(&self) -> Result<redb::Range<'_, K, V>, AppError>;
    fn range<'a, KR: Borrow<K::SelfType<'a>>>(&self, range: impl RangeBounds<KR>) -> Result<redb::Range<'static, K, V>, AppError>;
    fn index_range<'a, KR: Borrow<V::SelfType<'a>>>(&self, range: impl RangeBounds<KR>) -> Result<redb::MultimapRange<'static, V, K>, AppError>;
    fn last_key(&self) -> Result<Option<(AccessGuard<'_, K>, AccessGuard<'_, V>)>, AppError>;
    fn first_key(&self) -> Result<Option<(AccessGuard<'_, K>, AccessGuard<'_, V>)>, AppError>;
    fn stats(&self) -> Result<Vec<TableInfo>, AppError>;
}

pub trait WriteTableLike<K: CopyOwnedValue + 'static, V: Key + 'static> {
    fn insert_kv<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<(), AppError>;
    fn insert_many_sorted_by_key<'k, 'v, KR: Borrow<K::SelfType<'k>>, VR: Borrow<V::SelfType<'v>>>(&mut self, pairs: Vec<(KR, VR)>) -> Result<(), AppError>;
    fn delete_kv<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> Result<bool, AppError>;
    fn get_any_for_index<'v>(&mut self, value: impl Borrow<V::SelfType<'v>>) -> Result<Option<ValueOwned<K>>, AppError>;
    fn range<'a, KR: Borrow<K::SelfType<'a>> + 'a>(&self, range: impl RangeBounds<KR> + 'a) -> Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>;

    fn is_sorted_by_key<'k, 'v, KR: Borrow<K::SelfType<'k>>, VR: Borrow<V::SelfType<'v>>>(&self, pairs: &Vec<(KR, VR)>) -> bool {
        use std::cmp::Ordering;
        pairs.is_sorted_by(|(a, _), (b, _)| {
            matches!(K::compare(K::as_bytes(a.borrow()).as_ref(), K::as_bytes(b.borrow()).as_ref()), Ordering::Less)
        })
    }

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
    type ReadOnlyTable;
    fn name(&self) -> String;
    fn new_cache(&self) -> Self::CacheCtx;
    fn open_for_write<'txn, 'c>(&self, tx: &'txn WriteTransaction, cache: &'c mut Self::CacheCtx) -> Result<Self::Table<'txn, 'c>, AppError>;
    fn open_for_read(&self, db_weak: &Weak<Database>) -> redb::Result<Self::ReadOnlyTable, AppError>;
}

pub trait ReadTableFactory<K, V, KP, VP>: TableFactory<K, V>
where
    K: CopyOwnedValue + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    KP: KeyPartitioner<K> + Sync + Send + Clone + 'static,
    VP: ValuePartitioner<V> + Sync + Send + Clone + 'static,
{
    fn build_sharded_reader(
        &self,
        dbs: Vec<Weak<Database>>,
        part: &Partitioning<KP, VP>,
    ) -> Result<ShardedTableReader<K, V, KP, VP>, AppError>;
}

pub struct FlushState {
    pub sender: Option<Sender<Result<TaskResult, AppError>>>,
    pub sum: usize,
    pub shards: Option<usize>
}

pub enum WriterCommand<K: CopyOwnedValue + Send + 'static, V: Key + Send + 'static> {
    Begin(Sender<Result<(), AppError>>, Durability),
    WriteSortedInsertsOnFlush(Vec<(K, V)>),
    WriteInsertNow(K, V),
    AppendSortedInserts(Vec<(K, V)>),
    MergeUnsortedInserts(Vec<(K, V)>),
    Remove(K, Sender<Result<bool, AppError>>),
    QueryAndWrite {
        last_shards: Option<usize>,
        values: Vec<(usize, V)>,
        sink: Arc<dyn Fn(Option<usize>, Vec<(usize, Option<ValueOwned<K>>)>) -> Result<(), AppError> + Send + Sync + 'static>
    },
    Range(K, K, Sender<Result<Vec<(ValueBuf<K>, ValueBuf<V>)>, AppError>>),
    Flush(Sender<Result<TaskResult, AppError>>),
    FlushWhenReady { ack: Sender<Result<TaskResult, AppError>>, deferred: bool },
    ReadyForFlush(usize),
    Shutdown(Sender<Result<(), AppError>>),
}
pub struct WriteResult {
    pub collect_took: u128,
    pub sort_took: u128,
    pub write_took: u128
}

impl WriteResult {
    pub fn new(collect_took: u128, sort_took: u128, write_took: u128) -> Self {
        Self { collect_took, sort_took, write_took }
    }
}

pub enum Control {
    Continue,
    ReadyForFlush(usize),
    Commit(Sender<Result<TaskResult, AppError>>, Result<WriteResult, AppError>),
    FlushWhenReady(Sender<Result<TaskResult, AppError>>),
    Error(Sender<Result<TaskResult, AppError>>, AppError),
    Shutdown(Sender<Result<(), AppError>>),
    TxFinished,
}

pub trait WriterLike<K: CopyOwnedValue, V: Value> {
    fn acquire_router(&self) -> Arc<dyn Router<K, V>>;
    fn begin(&self, durability: Durability) -> Result<(), AppError>;
    fn begin_async(&self, durability: Durability) -> Result<Vec<StartFuture>, AppError>;
    fn insert_on_flush(&self, key: K, value: V) -> Result<(), AppError>;
    fn insert_now(&self, key: K, value: V) -> Result<(), AppError>;
    fn flush(&self) -> Result<TaskResult, AppError>;
    fn flush_async(&self) -> Result<Vec<FlushFuture>, AppError>;
    fn flush_two_phased(&self) -> Result<Vec<FlushFuture>, AppError>;
    fn flush_deferred(&self) -> Result<Vec<FlushFuture>, AppError>;
    fn shutdown(self) -> Result<(), AppError> where Self: Sized;
    fn shutdown_async(self) -> Result<Vec<StopFuture>, AppError> where Self: Sized;
}

pub struct RedbitTableDefinition<K, V, F, KP, VP>
where
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: ReadTableFactory<K, V, KP, VP> + Send + Clone + 'static,
    KP: KeyPartitioner<K> + Sync + Send + Clone + 'static,
    VP: ValuePartitioner<V> + Sync + Send + Clone + 'static,
{
    partitioning: Partitioning<KP, VP>,
    factory: F,
    k_phantom: PhantomData<K>,
    v_phantom: PhantomData<V>,
}

impl<K, V, F, KP, VP> ToReadField for RedbitTableDefinition<K, V, F, KP, VP>
where
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: ReadTableFactory<K, V, KP, VP> + Send + Clone + 'static,
    KP: KeyPartitioner<K> + Sync + Send + Clone + 'static,
    VP: ValuePartitioner<V> + Sync + Send + Clone + 'static,
{
    type ReadField = ShardedTableReader<K, V, KP, VP>;

    fn to_read_field(&self, storage: &Arc<Storage>) -> redb::Result<Self::ReadField, AppError> {
        self.reader(storage)
    }
}
impl<K, V, F, KP, VP> ToWriteField for RedbitTableDefinition<K, V, F, KP, VP>
where
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: ReadTableFactory<K, V, KP, VP> + Send + Clone + 'static,
    KP: KeyPartitioner<K> + Sync + Send + Clone + 'static,
    VP: ValuePartitioner<V> + Sync + Send + Clone + 'static,
{
    type WriteField = ShardedTableWriter<K, V, F, KP, VP>;

    fn to_write_field(&self, storage: &Arc<Storage>) -> redb::Result<Self::WriteField, AppError> {
        self.writer(storage)
    }
}

impl<K, V, F, KP, VP> RedbitTableDefinition<K, V, F, KP, VP>
where
    K: CopyOwnedValue + Send + 'static + Borrow<K::SelfType<'static>>,
    V: Key + Send + 'static + Borrow<V::SelfType<'static>>,
    F: ReadTableFactory<K, V, KP, VP> + Send + Clone + 'static,
    KP: KeyPartitioner<K> + Sync + Send + Clone + 'static,
    VP: ValuePartitioner<V> + Sync + Send + Clone + 'static,
{
    pub fn new(partitioning: Partitioning<KP, VP>, factory: F) -> Self {
        Self {
            partitioning,
            factory,
            k_phantom: PhantomData,
            v_phantom: PhantomData,
        }
    }

    pub fn writer_from_dbs(&self, dbs: Vec<Weak<Database>>) -> Result<ShardedTableWriter<K,V,F,KP,VP>, AppError> {
        let mut shards = Vec::with_capacity(dbs.len());
        for db_weak in dbs.into_iter() {
            shards.push(TxFSM::<K,V,F>::new(db_weak, self.factory.clone())?);
        }
        let senders: Vec<_> = shards.iter().map(|w| w.sender()).collect();
        let router = Arc::new(ShardedRouter::new(self.partitioning.clone(), senders));
        let deferred = AtomicBool::new(false);
        ShardedTableWriter::new(shards, router, deferred)
    }

    pub fn writer(&self, storage: &Arc<Storage>) -> Result<ShardedTableWriter<K,V,F,KP,VP>, AppError> {
        let dbs = storage.fetch_dbs(self.factory.name().as_str())?;
        self.writer_from_dbs(dbs)
    }

    pub fn reader_from_dbs(&self, dbs: Vec<Weak<Database>>) -> Result<ShardedTableReader<K, V, KP, VP>, AppError> {
        if dbs.len() < 1 {
            return Err(AppError::Custom(format!(
                "ShardedReadOnlyTable expected at least one database, got {}",
                dbs.len()
            )));
        }
        self.factory.build_sharded_reader(dbs, &self.partitioning)
    }

    pub fn reader(&self, storage: &Arc<Storage>) -> Result<ShardedTableReader<K, V, KP, VP>, AppError> {
        let dbs = storage.fetch_dbs(self.factory.name().as_str())?;
        self.reader_from_dbs(dbs)
    }
}

pub enum ShardedTableReader<K, V, KP, VP>
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    KP: KeyPartitioner<K> + Sync + Send + Clone + 'static,
    VP: ValuePartitioner<V> + Sync + Send + Clone + 'static,
{
    Plain(ShardedReadOnlyPlainTable<K, V, KP>),
    Index(ShardedReadOnlyIndexTable<K, V, VP>),
    Dict(ShardedReadOnlyDictTable<K, V, VP>),
}

impl<K, V, KP, VP> ReadTableLike<K, V> for ShardedTableReader<K, V, KP, VP>
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
    V: Key + 'static + Borrow<V::SelfType<'static>>,
    KP: KeyPartitioner<K> + Sync + Send + Clone + 'static,
    VP: ValuePartitioner<V> + Sync + Send + Clone + 'static,
{
    fn index_keys<'v>(&self, val: impl Borrow<V::SelfType<'v>>) -> Result<MultimapValue<'static, K>, AppError> {
        match self {
            ShardedTableReader::Index(t) => t.index_keys(val),
            _ => Err(AppError::Custom("index_keys unsupported for this table kind".into())),
        }
    }

    fn dict_keys<'v>(&self, val: impl Borrow<V::SelfType<'v>>) -> Result<Option<MultimapValue<'static, K>>, AppError> {
        match self {
            ShardedTableReader::Dict(t) => t.dict_keys(val),
            _ => Err(AppError::Custom("dict_keys unsupported for this table kind".into())),
        }
    }

    fn get_value<'k>(&self, key: impl Borrow<K::SelfType<'k>>) -> Result<Option<AccessGuard<'_, V>>, AppError> {
        match self {
            ShardedTableReader::Plain(t) => t.get_value(key),
            ShardedTableReader::Index(t) => t.get_value(key),
            ShardedTableReader::Dict(t) => t.get_value(key),
        }
    }

    fn iter_keys(&self) -> Result<redb::Range<'_, K, V>, AppError> {
        match self {
            ShardedTableReader::Plain(t) => t.iter_keys(),
            ShardedTableReader::Index(t) => t.iter_keys(),
            ShardedTableReader::Dict(t) => t.iter_keys(),
        }
    }

    fn range<'a, KR: Borrow<K::SelfType<'a>>>(&self, r: impl RangeBounds<KR>) -> Result<redb::Range<'static, K, V>, AppError> {
        match self {
            ShardedTableReader::Plain(t) => t.range(r),
            ShardedTableReader::Index(t) => t.range(r),
            ShardedTableReader::Dict(t) => t.range(r),
        }
    }

    fn index_range<'a, KR: Borrow<V::SelfType<'a>>>(&self, r: impl RangeBounds<KR>) -> Result<redb::MultimapRange<'static, V, K>, AppError> {
        match self {
            ShardedTableReader::Index(t) => t.index_range(r),
            _ => Err(AppError::Custom("index_range unsupported for this table kind".into())),
        }
    }

    fn first_key(&self) -> Result<Option<(AccessGuard<'_, K>, AccessGuard<'_, V>)>, AppError> {
        match self {
            ShardedTableReader::Plain(t) => t.first_key(),
            ShardedTableReader::Index(t) => t.first_key(),
            ShardedTableReader::Dict(t) => t.first_key(),
        }
    }

    fn last_key(&self) -> Result<Option<(AccessGuard<'_, K>, AccessGuard<'_, V>)>, AppError> {
        match self {
            ShardedTableReader::Plain(t) => t.last_key(),
            ShardedTableReader::Index(t) => t.last_key(),
            ShardedTableReader::Dict(t) => t.last_key(),
        }
    }

    fn stats(&self) -> Result<Vec<TableInfo>, AppError> {
        match self {
            ShardedTableReader::Plain(t) => t.stats(),
            ShardedTableReader::Index(t) => t.stats(),
            ShardedTableReader::Dict(t) => t.stats(),
        }
    }
}
