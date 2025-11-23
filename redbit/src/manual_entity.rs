use crate::entity_model::LoadResult;
use crate::error::AppError;
use crate::schema::{ColumnDef, ColumnProps, TypeInfo};
use crate::storage::context::{ToReadField, ToWriteField};
use crate::storage::init::{DbDef, Storage, StorageOwner};
use crate::storage::partitioning::{BytesPartitioner, Partitioning, Xxh3Partitioner};
use crate::storage::table_dict::DictFactory;
use crate::storage::table_index::IndexFactory;
use crate::storage::table_plain::PlainFactory;
use crate::storage::table_writer_api::{ReadTableLike, RedbitTableDefinition, WriterLike};
use crate::{CacheKey, ShardedTableReader, ShardedTableWriter};
use crate::{FlushFuture, StartFuture, StopFuture, WriteComponentRef};
use redb::{Durability, MultimapTableDefinition, TableDefinition};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::{env, fs};

/// Binds a logical table to its redb definition and cache parameters.
#[allow(dead_code)]
pub struct PlainTableBinding<K: crate::DbKey + Send + Sync, V: crate::DbVal + Send + Sync + Clone> {
    pub db_name: String,
    table_name: String,
    column_props: ColumnProps,
    redbit_def: RedbitTableDefinition<K, V, BytesPartitioner, Xxh3Partitioner, PlainFactory<K, V>>,
    root_pk: bool,
}

impl<K: crate::DbKey + Send + Sync, V: crate::DbVal + Send + Sync + Clone> PlainTableBinding<K, V> {
    /// Creates a PK table binding with db/table names derived from entity + pk field.
    pub fn pk(entity: &str, pk_name: &str, column_props: ColumnProps, is_root: bool) -> Self {
        let table_name = format!("{}_{}", entity.to_uppercase(), pk_name.to_uppercase());
        Self::new(&table_name, column_props, is_root)
    }

    /// Creates a plain column binding keyed by pk.
    pub fn plain(entity: &str, column: &str, pk_name: &str, column_props: ColumnProps) -> Self {
        let table_name = format!(
            "{}_{}_BY_{}",
            entity.to_uppercase(),
            column.to_uppercase(),
            pk_name.to_uppercase()
        );
        Self::new(&table_name, column_props, false)
    }

    fn new(table_name: &str, column_props: ColumnProps, is_root: bool) -> Self {
        let db_name = table_name.to_lowercase();
        let name_static: &'static str = Box::leak(table_name.to_string().into_boxed_str());
        let table_def = TableDefinition::<'static, K, V>::new(name_static);
        let redbit_def = RedbitTableDefinition::new(
            is_root,
            Partitioning::by_key(column_props.shards),
            PlainFactory::new(&db_name, table_def),
        );
        PlainTableBinding {
            db_name,
            table_name: table_name.to_string(),
            column_props,
            redbit_def,
            root_pk: is_root,
        }
    }

    /// Builds the DbDef used to open underlying shard databases.
    pub fn db_def(&self) -> DbDef {
        DbDef {
            name: self.db_name.clone(),
            shards: self.column_props.shards,
            db_cache_weight_or_zero: self.column_props.db_cache_weight,
            lru_cache_size_or_zero: self.column_props.lru_cache_size,
        }
    }

    /// Opens a writer for this table in the given storage.
    pub fn writer(&self, storage: &Arc<Storage>) -> redb::Result<ShardedTableWriter<K, V, BytesPartitioner, Xxh3Partitioner, PlainFactory<K, V>>, AppError> {
        self.redbit_def.to_write_field(storage)
    }

    /// Opens a reader for this table in the given storage.
    pub fn reader(&self, storage: &Arc<Storage>) -> redb::Result<ShardedTableReader<K, V, BytesPartitioner, Xxh3Partitioner>, AppError> {
        self.redbit_def.to_read_field(storage)
    }

    /// Ensures the table exists by opening a writer and flushing an empty batch.
    pub fn ensure_table(&self, storage: &Arc<Storage>) -> Result<(), AppError> {
        let writer = self.writer(storage)?;
        writer.begin(Durability::None)?;
        writer.flush()?;
        writer.shutdown()?;
        Ok(())
    }
}

/// Column runtime able to store and load one field without procedural macros.
pub trait ColumnRuntime<E: 'static + Clone, K>: Send + Sync
where
    K: crate::DbKey + Copy + Send + Sync + 'static,
{
    fn name(&self) -> &'static str;
    fn type_info(&self) -> TypeInfo;
    fn kind(&self) -> RuntimeKind;
    /// Optional static column name used for writer pooling / introspection.
    fn column_name(&self) -> Option<&'static str> { None }
    fn db_defs(&self) -> Vec<DbDef>;
    fn ensure_table(&self, storage: &Arc<Storage>) -> Result<(), AppError>;
    fn store(&self, storage: &Arc<Storage>, entity: &E, pk: K) -> Result<(), AppError>;
    fn load(&self, storage: &Arc<Storage>, pk: K) -> Result<LoadResult<E>, AppError>;
    fn store_many_fast(&self, _storage: &Arc<Storage>, _items: &[(K, E)], _durability: Durability) -> Result<bool, AppError> {
        Ok(false)
    }
    /// Optional open batch writer for pooling in contexts.
    fn begin_batch(&self, _storage: &Arc<Storage>) -> Result<Option<Box<dyn ColumnBatch<E, K>>>, AppError> {
        Ok(None)
    }
    fn store_many(&self, storage: &Arc<Storage>, items: &[(K, E)]) -> Result<(), AppError> {
        for (pk, e) in items {
            self.store(storage, e, *pk)?;
        }
        Ok(())
    }
    /// Cascade-aware batched store that may reuse pooled child writers when available.
    fn store_many_with_children(&self, storage: &Arc<Storage>, items: &[(K, E)], child_writers: Option<&dyn ChildWriters>, parent_writers: Option<&RuntimeWriters<E, K>>) -> Result<(), AppError> {
        let _ = child_writers;
        let _ = parent_writers;
        self.store_many(storage, items)
    }
}

/// Column batch that can insert multiple rows while exposing begin/commit hooks.
pub trait ColumnBatch<E, K>: WriteComponentRef + Send
where
    K: crate::DbKey + Copy + Send + Sync + 'static,
{
    fn insert(&self, pk: K, entity: &E) -> Result<(), AppError>;
    fn shutdown_async(self: Box<Self>) -> Result<Vec<StopFuture>, AppError>;
    fn as_any(&self) -> &dyn std::any::Any;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Plain,
    Index,
    Dict,
    Relationship,
    OneToOneCascade,
    OneToManyCascade,
    WriteFrom,
}

/// Plain column runtime storing values in a single table keyed by pk.
pub struct PlainColumnRuntime<
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: crate::DbVal + Clone + Send + Sync + 'static,
> where for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static {
    binding: PlainTableBinding<K, V>,
    getter: fn(&E) -> V,
    setter: fn(E, V) -> E,
    name: &'static str,
    tpe: TypeInfo,
}

impl<
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: crate::DbVal + Clone + Send + Sync + 'static,
> PlainColumnRuntime<E, K, V>
where
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    /// Constructs a plain column runtime with provided accessors.
    pub fn new(
        entity: &str,
        column: &'static str,
        pk_name: &str,
        column_props: ColumnProps,
        getter: fn(&E) -> V,
        setter: fn(E, V) -> E,
    ) -> Self {
        let binding = PlainTableBinding::plain(entity, column, pk_name, column_props);
        PlainColumnRuntime { binding, getter, setter, name: column, tpe: TypeInfo::of::<V>() }
    }
}


/// Binds an index table (value -> pk + pk -> value) to storage.
#[allow(dead_code)]
pub struct IndexTableBinding<K: crate::DbKey + Send + Sync, V: CacheKey + Send + Sync + Clone> {
    pub db_name: String,
    column_props: ColumnProps,
    redbit_def: RedbitTableDefinition<K, V, BytesPartitioner, Xxh3Partitioner, IndexFactory<K, V>>,
}

impl<K: crate::DbKey + Send + Sync, V: CacheKey + Send + Sync + Clone> IndexTableBinding<K, V> {
    pub fn new(entity: &str, column: &str, pk_name: &str, column_props: ColumnProps) -> Self {
        let index_table = format!("{}_{}_INDEX", entity.to_uppercase(), column.to_uppercase());
        let index_by_pk_table = format!(
            "{}_{}_BY_{}",
            entity.to_uppercase(),
            column.to_uppercase(),
            pk_name.to_uppercase()
        );
        let db_name = index_table.to_lowercase();
        let pk_by_index_def: MultimapTableDefinition<'static, V, K> = MultimapTableDefinition::new(Box::leak(index_table.into_boxed_str()));
        let index_by_pk_def: TableDefinition<'static, K, V> = TableDefinition::new(Box::leak(index_by_pk_table.into_boxed_str()));
        let redbit_def = RedbitTableDefinition::new(
            false,
            Partitioning::by_value(column_props.shards),
            IndexFactory::new(&db_name, column_props.lru_cache_size, pk_by_index_def, index_by_pk_def),
        );
        Self { db_name, column_props, redbit_def }
    }

    pub fn db_def(&self) -> DbDef {
        DbDef {
            name: self.db_name.clone(),
            shards: self.column_props.shards,
            db_cache_weight_or_zero: self.column_props.db_cache_weight,
            lru_cache_size_or_zero: self.column_props.lru_cache_size,
        }
    }

    pub fn writer(&self, storage: &Arc<Storage>) -> redb::Result<ShardedTableWriter<K, V, BytesPartitioner, Xxh3Partitioner, IndexFactory<K, V>>, AppError> {
        self.redbit_def.to_write_field(storage)
    }

    pub fn reader(&self, storage: &Arc<Storage>) -> redb::Result<ShardedTableReader<K, V, BytesPartitioner, Xxh3Partitioner>, AppError> {
        self.redbit_def.to_read_field(storage)
    }

    pub fn ensure_table(&self, storage: &Arc<Storage>) -> Result<(), AppError> {
        let writer = self.writer(storage)?;
        writer.begin(Durability::None)?;
        writer.flush()?;
        writer.shutdown()?;
        Ok(())
    }
}

/// Index column runtime storing a secondary index keyed by pk.
pub struct IndexColumnRuntime<
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
> where for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static {
    binding: IndexTableBinding<K, V>,
    getter: fn(&E) -> V,
    setter: fn(E, V) -> E,
    name: &'static str,
    tpe: TypeInfo,
    used: bool,
}

impl<
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
> IndexColumnRuntime<E, K, V>
where
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    pub fn new(
        entity: &str,
        column: &'static str,
        column_props: ColumnProps,
        pk_name: &str,
        getter: fn(&E) -> V,
        setter: fn(E, V) -> E,
    ) -> Self {
        let binding = IndexTableBinding::new(entity, column, pk_name, column_props);
        Self { binding, getter, setter, name: column, tpe: TypeInfo::of::<V>(), used: false }
    }

    pub fn new_used(
        entity: &str,
        column: &'static str,
        column_props: ColumnProps,
        pk_name: &str,
        getter: fn(&E) -> V,
        setter: fn(E, V) -> E,
    ) -> Self {
        let mut rt = Self::new(entity, column, column_props, pk_name, getter, setter);
        rt.used = true;
        rt
    }
}


impl<
    E: 'static + Clone,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
> ColumnRuntime<E, K> for IndexColumnRuntime<E, K, V>
where
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    fn name(&self) -> &'static str { self.name }
    fn column_name(&self) -> Option<&'static str> { Some(self.name) }
    fn type_info(&self) -> TypeInfo { self.tpe }
    fn kind(&self) -> RuntimeKind { RuntimeKind::Index }
    fn db_defs(&self) -> Vec<DbDef> { vec![self.binding.db_def()] }
    fn ensure_table(&self, storage: &Arc<Storage>) -> Result<(), AppError> {
        self.binding.ensure_table(storage)
    }
    fn store(&self, storage: &Arc<Storage>, entity: &E, pk: K) -> Result<(), AppError> {
        let val = (self.getter)(entity);
        let writer = self.binding.writer(storage)?;
        writer.begin(Durability::None)?;
        if self.used {
            writer.insert_now(pk, val)?;
        } else {
            writer.insert_on_flush(pk, val)?;
        }
        writer.flush()?;
        writer.shutdown()?;
        Ok(())
    }
    fn begin_batch(&self, storage: &Arc<Storage>) -> Result<Option<Box<dyn ColumnBatch<E, K>>>, AppError> {
        let writer = self.binding.writer(storage)?;
        Ok(Some(Box::new(IndexBatch { writer, getter: self.getter, used: self.used })))
    }
    fn store_many(&self, storage: &Arc<Storage>, items: &[(K, E)]) -> Result<(), AppError> {
        if items.is_empty() { return Ok(()); }
        let writer = self.binding.writer(storage)?;
        writer.begin(Durability::None)?;
        for (pk, e) in items {
            let val = (self.getter)(e);
            if self.used {
                writer.insert_now(*pk, val)?;
            } else {
                writer.insert_on_flush(*pk, val)?;
            }
        }
        writer.flush()?;
        writer.shutdown()?;
        Ok(())
    }

    fn load(&self, storage: &Arc<Storage>, pk: K) -> Result<LoadResult<E>, AppError> {
        let setter = self.setter;
        let reader = self.binding.reader(storage)?;
        match reader.get_value(pk)? {
            Some(guard) => {
                let owned: V = guard.value().into();
                drop(guard);
                Ok(LoadResult::Value(Box::new(move |mut e: E| {
                    e = setter(e, owned);
                    e
                })))
            }
            None => Ok(LoadResult::Reject),
        }
    }
}

pub struct IndexBatch<E, K, V>
where
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
{
    pub writer: ShardedTableWriter<K, V, BytesPartitioner, Xxh3Partitioner, IndexFactory<K, V>>,
    getter: fn(&E) -> V,
    used: bool,
}

impl<E, K, V> WriteComponentRef for IndexBatch<E, K, V>
where
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    fn begin_async_ref(&self, d: Durability) -> redb::Result<Vec<StartFuture>, AppError> { self.writer.begin_async(d) }
    fn commit_with_ref(&self) -> Result<Vec<FlushFuture>, AppError> { self.writer.flush_async() }
}

impl<E, K, V> ColumnBatch<E, K> for IndexBatch<E, K, V>
where
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    fn insert(&self, pk: K, entity: &E) -> Result<(), AppError> {
        let val = (self.getter)(entity);
        if self.used {
            self.writer.insert_now(pk, val)
        } else {
            self.writer.insert_on_flush(pk, val)
        }
    }

    fn shutdown_async(self: Box<Self>) -> Result<Vec<crate::storage::table_writer_api::StopFuture>, AppError> { self.writer.shutdown_async() }

    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl<
    E: 'static + Clone,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: crate::DbVal + Clone + Send + Sync + 'static,
> ColumnRuntime<E, K> for PlainColumnRuntime<E, K, V>
where
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    fn name(&self) -> &'static str {
        self.name
    }
    fn column_name(&self) -> Option<&'static str> { Some(self.name) }
    fn type_info(&self) -> TypeInfo { self.tpe }
    fn kind(&self) -> RuntimeKind { RuntimeKind::Plain }

    fn db_defs(&self) -> Vec<DbDef> {
        vec![self.binding.db_def()]
    }

    fn ensure_table(&self, storage: &Arc<Storage>) -> Result<(), AppError> {
        let writer = self.binding.writer(storage)?;
        writer.begin(Durability::None)?;
        writer.flush()?;
        writer.shutdown()?;
        Ok(())
    }

    fn store(&self, storage: &Arc<Storage>, entity: &E, pk: K) -> Result<(), AppError> {
        let val = (self.getter)(entity);
        let writer = self.binding.writer(storage)?;
        writer.begin(Durability::None)?;
        writer.insert_on_flush(pk, val)?;
        writer.flush()?;
        writer.shutdown()?;
        Ok(())
    }
    fn store_many(&self, storage: &Arc<Storage>, items: &[(K, E)]) -> Result<(), AppError> {
        if items.is_empty() { return Ok(()); }
        let writer = self.binding.writer(storage)?;
        writer.begin(Durability::None)?;
        for (pk, e) in items {
            let val = (self.getter)(e);
            writer.insert_on_flush(*pk, val)?;
        }
        writer.flush()?;
        writer.shutdown()?;
        Ok(())
    }

    fn load(&self, storage: &Arc<Storage>, pk: K) -> Result<LoadResult<E>, AppError> {
        let reader = self.binding.reader(storage)?;
        match reader.get_value(pk)? {
            Some(guard) => {
                let setter = self.setter;
                let owned: V = guard.value().into();
                drop(guard);
                Ok(LoadResult::Value(Box::new(move |mut e: E| {
                    e = setter(e, owned);
                    e
                })))
            }
            None => Ok(LoadResult::Reject),
        }
    }

    fn begin_batch(&self, storage: &Arc<Storage>) -> Result<Option<Box<dyn ColumnBatch<E, K>>>, AppError> {
        let writer = self.binding.writer(storage)?;
        Ok(Some(Box::new(PlainBatch { writer, getter: self.getter })))
    }
}

struct DictBatch<E, K, V>
where
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
{
    pub writer: ShardedTableWriter<K, V, BytesPartitioner, Xxh3Partitioner, DictFactory<K, V>>,
    getter: fn(&E) -> V,
}

impl<E, K, V> WriteComponentRef for DictBatch<E, K, V>
where
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    fn begin_async_ref(&self, d: Durability) -> redb::Result<Vec<StartFuture>, AppError> { self.writer.begin_async(d) }
    fn commit_with_ref(&self) -> Result<Vec<FlushFuture>, AppError> { self.writer.flush_async() }
}

impl<E, K, V> ColumnBatch<E, K> for DictBatch<E, K, V>
where
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    fn insert(&self, pk: K, entity: &E) -> Result<(), AppError> {
        let val = (self.getter)(entity);
        self.writer.insert_on_flush(pk, val)
    }

    fn shutdown_async(self: Box<Self>) -> Result<Vec<crate::storage::table_writer_api::StopFuture>, AppError> {
        self.writer.shutdown_async()
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}

pub struct PlainBatch<E, K, V>
where
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: crate::DbVal + Clone + Send + Sync + 'static,
{
    pub writer: ShardedTableWriter<K, V, BytesPartitioner, Xxh3Partitioner, PlainFactory<K, V>>,
    getter: fn(&E) -> V,
}

impl<E, K, V> WriteComponentRef for PlainBatch<E, K, V>
where
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: crate::DbVal + Clone + Send + Sync + 'static,
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    fn begin_async_ref(&self, d: Durability) -> redb::Result<Vec<StartFuture>, AppError> { self.writer.begin_async(d) }
    fn commit_with_ref(&self) -> Result<Vec<FlushFuture>, AppError> { self.writer.flush_async() }
}

impl<E, K, V> ColumnBatch<E, K> for PlainBatch<E, K, V>
where
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: crate::DbVal + Clone + Send + Sync + 'static,
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    fn insert(&self, pk: K, entity: &E) -> Result<(), AppError> {
        let val = (self.getter)(entity);
        self.writer.insert_on_flush(pk, val)
    }

    fn shutdown_async(self: Box<Self>) -> Result<Vec<crate::storage::table_writer_api::StopFuture>, AppError> { self.writer.shutdown_async() }

    fn as_any(&self) -> &dyn std::any::Any { self }
}

/// Minimal manual entity runtime for pk + plain columns.
pub struct ManualEntityRuntime<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> {
    pub name: &'static str,
    pk_binding: PlainTableBinding<K, ()>,
    pk_getter: fn(&E) -> K,
    seed_with_key: fn(&K) -> E,
    columns: Vec<Box<dyn ColumnRuntime<E, K>>>,
}

/// Open writers for a manual runtime to reuse across batches.
pub struct RuntimeWriters<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> {
    pub pk_writer: ShardedTableWriter<K, (), BytesPartitioner, Xxh3Partitioner, PlainFactory<K, ()>>,
    pub column_batches: Vec<Option<Box<dyn ColumnBatch<E, K>>>>,
    pub labels: Vec<&'static str>,
}

impl<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> RuntimeWriters<E, K> {
    pub fn writer_refs(&self) -> Vec<&dyn WriteComponentRef> {
        let mut out: Vec<&dyn WriteComponentRef> = Vec::new();
        out.push(&self.pk_writer);
        for b in &self.column_batches {
            if let Some(b) = b.as_ref() {
                out.push(&**b);
            }
        }
        out
    }

    pub fn writer_refs_labeled(&self) -> Vec<(&'static str, &dyn WriteComponentRef)> {
        let mut out: Vec<(&'static str, &dyn WriteComponentRef)> = Vec::new();
        out.push(("pk", &self.pk_writer as &dyn WriteComponentRef));
        for (idx, batch) in self.column_batches.iter().enumerate() {
            if let Some(b) = batch.as_ref() {
                let label = self.labels.get(idx + 1).copied().unwrap_or("col");
                out.push((label, &**b));
            }
        }
        out
    }

    pub fn stop_async(self) -> Result<Vec<StopFuture>, AppError> {
        let mut stops = Vec::new();
        stops.extend(self.pk_writer.shutdown_async()?);
        for b in self.column_batches {
            if let Some(batch) = b {
                stops.extend(batch.shutdown_async()?);
            }
        }
        Ok(stops)
    }
}

/// Nested writer tree to pool child runtimes alongside a parent.
pub struct RuntimeWritersWithChildren<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> {
    pub self_writers: RuntimeWriters<E, K>,
    pub children: Vec<( &'static str, Box<dyn ChildWriters> )>,
}

impl<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> RuntimeWritersWithChildren<E, K> {
    pub fn writer_refs(&self) -> Vec<&dyn WriteComponentRef> {
        fn collect<'a>(acc: &mut Vec<&'a dyn WriteComponentRef>, child: &'a dyn ChildWriters) {
            acc.extend(child.writer_refs());
            for grand in child.children_refs() {
                collect(acc, grand);
            }
        }

        let mut refs = self.self_writers.writer_refs();
        for (_, c) in &self.children {
            collect(&mut refs, &**c);
        }
        refs
    }

    pub fn writer_refs_labeled(&self) -> Vec<(&'static str, &dyn WriteComponentRef)> {
        let mut out = self.self_writers.writer_refs_labeled();
        for (_, c) in &self.children {
            out.extend(c.writer_refs_labeled());
        }
        out
    }

    /// Begin all writers in this tree with provided durability.
    pub fn begin_all(&self, d: redb::Durability) -> Result<Vec<StartFuture>, AppError> {
        let mut starts = Vec::new();
        let refs = self.writer_refs_labeled();
        for (_, c) in refs {
            starts.extend(c.begin_async_ref(d)?);
        }
        Ok(starts)
    }

    /// Commit all writers in this tree (Flush/FlushWhenReady honoring deferred routers).
    pub fn commit_all(&self) -> Result<Vec<FlushFuture>, AppError> {
        let mut flushes = Vec::new();
        for (_, c) in self.writer_refs_labeled() {
            flushes.extend(c.commit_with_ref()?);
        }
        Ok(flushes)
    }

    fn labels(&self) -> Vec<&'static str> {
        let mut labs = self.self_writers.labels.clone();
        for (name, child) in &self.children {
            labs.push(*name);
            labs.extend(child.children_labels());
        }
        labs
    }

    pub fn stop_async(self) -> Result<Vec<StopFuture>, AppError> {
        let mut stops = self.self_writers.stop_async()?;
        for (_, c) in self.children {
            stops.extend(c.stop_async()?);
        }
        Ok(stops)
    }
}

/// Trait used to erase child writer trees for pooling.
pub trait ChildWriters: Send {
    fn writer_refs(&self) -> Vec<&dyn WriteComponentRef>;
    fn writer_refs_labeled(&self) -> Vec<(&'static str, &dyn WriteComponentRef)>;
    fn stop_async(self: Box<Self>) -> Result<Vec<StopFuture>, AppError>;
    fn as_any(&self) -> &dyn Any;
    fn children_refs(&self) -> Vec<&dyn ChildWriters>;
    fn children_labels(&self) -> Vec<&'static str>;
}

impl<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> ChildWriters for RuntimeWritersWithChildren<E, K> {
    fn writer_refs(&self) -> Vec<&dyn WriteComponentRef> { self.self_writers.writer_refs() }
    fn writer_refs_labeled(&self) -> Vec<(&'static str, &dyn WriteComponentRef)> {
        let mut out = self.self_writers.writer_refs_labeled();
        for (_, c) in &self.children {
            out.extend(c.writer_refs_labeled());
        }
        out
    }
    fn stop_async(self: Box<Self>) -> Result<Vec<StopFuture>, AppError> { RuntimeWritersWithChildren::stop_async(*self) }
    fn as_any(&self) -> &dyn Any { self }
    fn children_refs(&self) -> Vec<&dyn ChildWriters> { self.children.iter().map(|(_, c)| &**c as &dyn ChildWriters).collect() }
    fn children_labels(&self) -> Vec<&'static str> { self.labels() }
}

impl<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> ChildWriters for RuntimeWriters<E, K> {
    fn writer_refs(&self) -> Vec<&dyn WriteComponentRef> { RuntimeWriters::writer_refs(self) }
    fn writer_refs_labeled(&self) -> Vec<(&'static str, &dyn WriteComponentRef)> {
        self.labels.iter().cloned().zip(self.writer_refs().into_iter()).collect()
    }
    fn stop_async(self: Box<Self>) -> Result<Vec<StopFuture>, AppError> { RuntimeWriters::stop_async(*self) }
    fn as_any(&self) -> &dyn Any { self }
    fn children_refs(&self) -> Vec<&dyn ChildWriters> { Vec::new() }
    fn children_labels(&self) -> Vec<&'static str> { Vec::new() }
}

impl<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> ManualEntityRuntime<E, K> {
    /// Builds a new manual entity runtime from pk binding and column runtimes.
    pub fn new(
        name: &'static str,
        pk_binding: PlainTableBinding<K, ()>,
        pk_getter: fn(&E) -> K,
        seed_with_key: fn(&K) -> E,
        columns: Vec<Box<dyn ColumnRuntime<E, K>>>,
    ) -> Self {
        ManualEntityRuntime { name, pk_binding, pk_getter, seed_with_key, columns }
    }

    /// Returns DbDefs for all bound tables to open storage manually.
    pub fn db_defs(&self) -> Vec<DbDef> {
        let mut defs = vec![self.pk_binding.db_def()];
        for c in &self.columns {
            defs.extend(c.db_defs());
        }
        defs
    }

    /// Stores a single entity by writing pk + each column.
    pub fn store(&self, storage: &Arc<Storage>, entity: &E) -> Result<(), AppError> {
        self.pk_binding.ensure_table(storage)?;
        for col in &self.columns {
            col.ensure_table(storage)?;
        }
        let pk = (self.pk_getter)(entity);
        let pk_writer = self.pk_binding.writer(storage)?;
        pk_writer.begin(Durability::None)?;
        pk_writer.insert_on_flush(pk, ())?;
        pk_writer.flush()?;
        pk_writer.shutdown()?;
        for col in &self.columns {
            col.store(storage, entity, pk)?;
        }
        Ok(())
    }

    pub fn store_batch_with_durability(&self, storage: &Arc<Storage>, entities: &[E], durability: Durability) -> Result<(), AppError> {
        if entities.is_empty() { return Ok(()); }
        self.pk_binding.ensure_table(storage)?;
        for col in &self.columns {
            col.ensure_table(storage)?;
        }
        let pk_writer = self.pk_binding.writer(storage)?;
        pk_writer.begin(durability)?;
        let mut pk_entities: Vec<(K, E)> = Vec::with_capacity(entities.len());
        for e in entities {
            let pk = (self.pk_getter)(e);
        pk_writer.insert_on_flush(pk, ())?;
        pk_entities.push((pk, e.clone()));
    }
    pk_writer.flush()?;
    pk_writer.shutdown()?;
    for col in &self.columns {
            col.store_many(storage, &pk_entities)?;
    }
    Ok(())
}

    pub fn store_batch(&self, storage: &Arc<Storage>, entities: &[E]) -> Result<(), AppError> {
        self.store_batch_with_durability(storage, entities, Durability::None)
    }

    /// Opens writers for pk and fast columns without beginning them (context handles begin/commit).
    pub fn begin_writers(&self, storage: &Arc<Storage>) -> Result<RuntimeWriters<E, K>, AppError> {
        // Ensure backing tables exist before pooling writers.
        self.pk_binding.ensure_table(storage)?;
        for col in &self.columns {
            col.ensure_table(storage)?;
        }
        let pk_writer = self.pk_binding.writer(storage)?;
        let mut column_batches = Vec::with_capacity(self.columns.len());
        let mut labels: Vec<&'static str> = Vec::with_capacity(self.columns.len() + 1);
        labels.push("pk");
        for (idx, col) in self.columns.iter().enumerate() {
            column_batches.push(col.begin_batch(storage)?);
            let label = col.column_name().unwrap_or_else(|| Box::leak(format!("col_{idx}").into_boxed_str()));
            labels.push(label);
        }
        Ok(RuntimeWriters { pk_writer, column_batches, labels })
    }
    /// Begin writers and wrap in a child-aware structure (children empty for now).
    pub fn begin_writers_with_children(&self, storage: &Arc<Storage>) -> Result<RuntimeWritersWithChildren<E, K>, AppError> {
        let self_writers = self.begin_writers(storage)?;
        Ok(RuntimeWritersWithChildren { self_writers, children: Vec::new() })
    }

    /// Store using pre-opened writers (assumed begun/committed by caller).
    pub fn store_batch_with_writers(
        &self,
        storage: &Arc<Storage>,
        writers: &RuntimeWriters<E, K>,
        entities: &[E],
    ) -> Result<(), AppError> {
        self.store_batch_with_writers_and_cascades(storage, writers, entities, None)
    }

    /// Store using pre-opened writer tree to enable cascade pooling.
    pub fn store_batch_with_writer_tree(
        &self,
        storage: &Arc<Storage>,
        writers: &RuntimeWritersWithChildren<E, K>,
        entities: &[E],
    ) -> Result<(), AppError> {
        let child_refs: Vec<(&'static str, &dyn ChildWriters)> = writers.children.iter().map(|(name, c)| (*name, &**c)).collect();
        self.store_batch_with_writers_and_cascades(storage, &writers.self_writers, entities, Some(&child_refs))
    }

    fn store_batch_with_writers_and_cascades(
        &self,
        storage: &Arc<Storage>,
        writers: &RuntimeWriters<E, K>,
        entities: &[E],
        child_writers: Option<&[(&'static str, &dyn ChildWriters)]>,
    ) -> Result<(), AppError> {
        if entities.is_empty() { return Ok(()); }
        let mut pk_entities: Vec<(K, E)> = Vec::with_capacity(entities.len());
        for e in entities {
            let pk = (self.pk_getter)(e);
            writers.pk_writer.insert_on_flush(pk, ())?;
            pk_entities.push((pk, e.clone()));
        }
        for (idx, col) in self.columns.iter().enumerate() {
            let child_writer = child_writers.and_then(|children| {
                let name = col.column_name()?;
                children.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
            });
            if let Some(batch) = writers.column_batches.get(idx).and_then(|b| b.as_ref()) {
                for (pk, e) in &pk_entities {
                    batch.insert(*pk, e)?;
                }
            } else {
                col.store_many_with_children(storage, &pk_entities, child_writer, Some(writers))?;
            }
        }
        Ok(())
    }

    /// Composes an entity by fetching pk presence and each column value.
    pub fn compose(&self, storage: &Arc<Storage>, pk: K) -> Result<Option<E>, AppError> {
        self.pk_binding.ensure_table(storage)?;
        for col in &self.columns {
            col.ensure_table(storage)?;
        }
        let pk_reader = self.pk_binding.reader(storage)?;
        if pk_reader.get_value(pk)?.is_none() {
            return Ok(None);
        }
        let mut entity = (self.seed_with_key)(&pk);
        for col in &self.columns {
            match col.load(storage, pk)? {
                LoadResult::Value(apply) => entity = apply(entity),
                LoadResult::Skip => {}
                LoadResult::Reject => return Ok(None),
            }
        }
        Ok(Some(entity))
    }
}

/// Convenient helpers for testing manual schemas.
pub struct ManualTestScope;

/// Builder that validates schema definitions against provided column runtimes.
pub(crate) struct RuntimeFactory<E: 'static, K: crate::DbKey + Copy + Send + Sync + 'static> {
    tpe: TypeInfo,
    kind: RuntimeKind,
    build: Box<dyn Fn(ColumnProps) -> Box<dyn ColumnRuntime<E, K>> + Send + Sync>,
}

/// Registry that can be derived from schema to auto-wire runtimes.
#[derive(Default)]
pub struct AccessorRegistry<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> {
    factories: HashMap<&'static str, RuntimeFactory<E, K>>,
}

/// Accessor is a type-erased registration produced by (future) derive macros.
pub enum Accessor<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> {
    Register(Box<dyn FnOnce(ManualRuntimeBuilder<E, K>) -> ManualRuntimeBuilder<E, K>>),
}

pub fn plain_accessor<E, K, V>(name: &'static str, getter: fn(&E) -> V, setter: fn(E, V) -> E) -> Accessor<E, K>
where
    E: 'static + Clone,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: crate::DbVal + Clone + Send + Sync + 'static,
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    Accessor::Register(Box::new(move |b| b.register_plain(name, getter, setter)))
}

pub fn index_accessor<E, K, V>(name: &'static str, getter: fn(&E) -> V, setter: fn(E, V) -> E) -> Accessor<E, K>
where
    E: 'static + Clone,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    Accessor::Register(Box::new(move |b| b.register_index(name, getter, setter)))
}

pub fn index_used_accessor<E, K, V>(name: &'static str, getter: fn(&E) -> V, setter: fn(E, V) -> E) -> Accessor<E, K>
where
    E: 'static + Clone,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    Accessor::Register(Box::new(move |b| b.register_index_used(name, getter, setter)))
}

pub fn dict_accessor<E, K, V>(name: &'static str, getter: fn(&E) -> V, setter: fn(E, V) -> E) -> Accessor<E, K>
where
    E: 'static + Clone,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    Accessor::Register(Box::new(move |b| b.register_dict(name, getter, setter)))
}

/// Write-from accessor that registers a runtime consuming existing writer batches.
pub fn write_from_accessor<E, K>(
    name: &'static str,
    tpe: TypeInfo,
    apply: std::sync::Arc<dyn Fn(&RuntimeWriters<E, K>, Option<&dyn ChildWriters>, &[(K, E)]) -> Result<(), AppError> + Send + Sync>,
) -> Accessor<E, K>
where
    E: 'static + Clone,
    K: crate::DbKey + Copy + Send + Sync + 'static,
{
    Accessor::Register(Box::new(move |b| {
        let runtime: Box<dyn ColumnRuntime<E, K>> = Box::new(WriteFromRuntime::new(name, tpe, apply.clone()));
        b.with_runtime(runtime)
    }))
}

pub fn relationship_accessor<E, K>(
    name: &'static str,
    tpe: TypeInfo,
    store_fn: Arc<dyn Fn(&Arc<Storage>, &E, K) -> Result<(), AppError> + Send + Sync>,
    load_fn: Arc<dyn Fn(&Arc<Storage>, K) -> Result<Option<Box<dyn FnOnce(E) -> E + Send>>, AppError> + Send + Sync>,
) -> Accessor<E, K>
where
    E: 'static + Clone,
    K: crate::DbKey + Copy + Send + Sync + 'static,
{
    Accessor::Register(Box::new(move |b| b.register_relationship(name, tpe, store_fn.clone(), load_fn.clone())))
}

pub fn one2one_cascade_accessor<P, C, PK, CK>(
    name: &'static str,
    child_runtime: Arc<ManualEntityRuntime<C, CK>>,
    pk_map: fn(PK) -> CK,
    getter: fn(&P) -> Option<C>,
    setter: fn(P, Option<C>) -> P,
) -> Accessor<P, PK>
where
    P: 'static + Clone,
    C: 'static + Clone + Send + Sync,
    PK: crate::DbKey + Copy + Send + Sync + 'static,
    CK: crate::DbKey + Copy + Send + Sync + 'static + PartialEq,
{
    let cr = child_runtime.clone();
    Accessor::Register(Box::new(move |b| {
        let runtime: Box<dyn ColumnRuntime<P, PK>> = Box::new(OneToOneCascadeRuntime::new(name, cr.clone(), pk_map, getter, setter));
        b.with_runtime(runtime)
    }))
}

pub fn one2many_cascade_accessor<P, C, PK, CK>(
    name: &'static str,
    child_runtime: Arc<ManualEntityRuntime<C, CK>>,
    child_pk_at: Arc<dyn Fn(PK, usize) -> CK + Send + Sync>,
    getter: fn(&P) -> Vec<C>,
    setter: fn(P, Vec<C>) -> P,
) -> Accessor<P, PK>
where
    P: 'static + Clone,
    C: 'static + Clone + Send + Sync,
    PK: crate::DbKey + Copy + Send + Sync + 'static,
    CK: crate::DbKey + Copy + Send + Sync + 'static + PartialEq,
{
    let cr = child_runtime.clone();
    Accessor::Register(Box::new(move |b| {
        let runtime: Box<dyn ColumnRuntime<P, PK>> = Box::new(OneToManyCascadeRuntime::new(name, cr.clone(), child_pk_at.clone(), getter, setter));
        b.with_runtime(runtime)
    }))
}

impl<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> AccessorRegistry<E, K> {
    pub fn register_plain<V>(
        mut self,
        name: &'static str,
        entity: &'static str,
        pk_name: &'static str,
        getter: fn(&E) -> V,
        setter: fn(E, V) -> E,
    ) -> Self
    where
        V: crate::DbVal + Clone + Send + Sync + 'static,
        for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
    {
        let factory = RuntimeFactory {
            tpe: TypeInfo::of::<V>(),
            kind: RuntimeKind::Plain,
            build: Box::new(move |props: ColumnProps| {
                Box::new(PlainColumnRuntime::new(entity, name, pk_name, props, getter, setter)) as Box<dyn ColumnRuntime<E, K>>
            }),
        };
        self.factories.insert(name, factory);
        self
    }

    pub fn register_index<V>(
        mut self,
        name: &'static str,
        entity: &'static str,
        pk_name: &'static str,
        getter: fn(&E) -> V,
        setter: fn(E, V) -> E,
    ) -> Self
    where
        V: CacheKey + Clone + Send + Sync + 'static,
        for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
    {
        let factory = RuntimeFactory {
            tpe: TypeInfo::of::<V>(),
            kind: RuntimeKind::Index,
            build: Box::new(move |props: ColumnProps| {
                Box::new(IndexColumnRuntime::new(entity, name, props, pk_name, getter, setter)) as Box<dyn ColumnRuntime<E, K>>
            }),
        };
        self.factories.insert(name, factory);
        self
    }

    pub fn register_dict<V>(
        mut self,
        name: &'static str,
        entity: &'static str,
        pk_name: &'static str,
        getter: fn(&E) -> V,
        setter: fn(E, V) -> E,
    ) -> Self
    where
        V: CacheKey + Clone + Send + Sync + 'static,
        for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
    {
        let factory = RuntimeFactory {
            tpe: TypeInfo::of::<V>(),
            kind: RuntimeKind::Dict,
            build: Box::new(move |props: ColumnProps| {
                Box::new(DictColumnRuntime::new(entity, name, pk_name, props, getter, setter)) as Box<dyn ColumnRuntime<E, K>>
            }),
        };
        self.factories.insert(name, factory);
        self
    }

    pub fn register_relationship(
        mut self,
        name: &'static str,
        tpe: TypeInfo,
        store_fn: Arc<dyn Fn(&Arc<Storage>, &E, K) -> Result<(), AppError> + Send + Sync>,
        load_fn: Arc<dyn Fn(&Arc<Storage>, K) -> Result<Option<Box<dyn FnOnce(E) -> E + Send>>, AppError> + Send + Sync>,
    ) -> Self {
        let factory = RuntimeFactory {
            tpe,
            kind: RuntimeKind::Relationship,
            build: Box::new(move |_props: ColumnProps| {
                Box::new(RelationshipRuntime::new(name, tpe, store_fn.clone(), load_fn.clone())) as Box<dyn ColumnRuntime<E, K>>
            }),
        };
        self.factories.insert(name, factory);
        self
    }

    pub(crate) fn into_factories(self) -> HashMap<&'static str, RuntimeFactory<E, K>> { self.factories }
}

pub struct ManualRuntimeBuilder<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> {
    name: &'static str,
    pk_binding: PlainTableBinding<K, ()>,
    pk_name: &'static str,
    pk_getter: fn(&E) -> K,
    seed_with_key: fn(&K) -> E,
    runtimes: Vec<Box<dyn ColumnRuntime<E, K>>>,
    registry: std::collections::HashMap<&'static str, RuntimeFactory<E, K>>,
}

impl<E: 'static + Clone, K: crate::DbKey + Copy + Send + Sync + 'static> ManualRuntimeBuilder<E, K> {
    pub fn new(
        name: &'static str,
        pk_binding: PlainTableBinding<K, ()>,
        pk_name: &'static str,
        pk_getter: fn(&E) -> K,
        seed_with_key: fn(&K) -> E,
    ) -> Self {
        Self { name, pk_binding, pk_name, pk_getter, seed_with_key, runtimes: Vec::new(), registry: HashMap::new() }
    }

    pub fn from_registry(
        name: &'static str,
        pk_binding: PlainTableBinding<K, ()>,
        pk_name: &'static str,
        pk_getter: fn(&E) -> K,
        seed_with_key: fn(&K) -> E,
        registry: AccessorRegistry<E, K>,
    ) -> Self {
        Self { name, pk_binding, pk_name, pk_getter, seed_with_key, runtimes: Vec::new(), registry: registry.into_factories() }
    }

    pub fn with_accessors(mut self, accessors: Vec<Accessor<E, K>>) -> Self {
        for accessor in accessors {
            match accessor {
                Accessor::Register(f) => {
                    self = f(self);
                }
            }
        }
        self
    }

    pub fn with_registry(mut self, registry: AccessorRegistry<E, K>) -> Self {
        self.registry = registry.into_factories();
        self
    }

    pub fn with_runtime(mut self, runtime: Box<dyn ColumnRuntime<E, K>>) -> Self {
        self.runtimes.push(runtime);
        self
    }

    pub fn register_plain<V>(
        mut self,
        name: &'static str,
        getter: fn(&E) -> V,
        setter: fn(E, V) -> E,
    ) -> Self
    where
        V: crate::DbVal + Clone + Send + Sync + 'static,
        for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
    {
        let entity = self.name;
        let pk_name = self.pk_name;
        let factory = RuntimeFactory {
            tpe: TypeInfo::of::<V>(),
            kind: RuntimeKind::Plain,
            build: Box::new(move |props: ColumnProps| {
                Box::new(PlainColumnRuntime::new(entity, name, pk_name, props, getter, setter)) as Box<dyn ColumnRuntime<E, K>>
            }),
        };
        self.registry.insert(name, factory);
        self
    }

    pub fn register_index<V>(
        mut self,
        name: &'static str,
        getter: fn(&E) -> V,
        setter: fn(E, V) -> E,
    ) -> Self
    where
        V: CacheKey + Clone + Send + Sync + 'static,
        for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
    {
        let entity = self.name;
        let pk_name = self.pk_name;
        let factory = RuntimeFactory {
            tpe: TypeInfo::of::<V>(),
            kind: RuntimeKind::Index,
            build: Box::new(move |props: ColumnProps| {
                Box::new(IndexColumnRuntime::new(entity, name, props, pk_name, getter, setter)) as Box<dyn ColumnRuntime<E, K>>
            }),
        };
        self.registry.insert(name, factory);
        self
    }

    pub fn register_index_used<V>(
        mut self,
        name: &'static str,
        getter: fn(&E) -> V,
        setter: fn(E, V) -> E,
    ) -> Self
    where
        V: CacheKey + Clone + Send + Sync + 'static,
        for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
    {
        let entity = self.name;
        let pk_name = self.pk_name;
        let factory = RuntimeFactory {
            tpe: TypeInfo::of::<V>(),
            kind: RuntimeKind::Index,
            build: Box::new(move |props: ColumnProps| {
                Box::new(IndexColumnRuntime::new_used(entity, name, props, pk_name, getter, setter)) as Box<dyn ColumnRuntime<E, K>>
            }),
        };
        self.registry.insert(name, factory);
        self
    }

    pub fn register_dict<V>(
        mut self,
        name: &'static str,
        getter: fn(&E) -> V,
        setter: fn(E, V) -> E,
    ) -> Self
    where
        V: CacheKey + Clone + Send + Sync + 'static,
        for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
    {
        let entity = self.name;
        let pk_name = self.pk_name;
        let factory = RuntimeFactory {
            tpe: TypeInfo::of::<V>(),
            kind: RuntimeKind::Dict,
            build: Box::new(move |props: ColumnProps| {
                Box::new(DictColumnRuntime::new(entity, name, pk_name, props, getter, setter)) as Box<dyn ColumnRuntime<E, K>>
            }),
        };
        self.registry.insert(name, factory);
        self
    }

    pub fn register_relationship(
        mut self,
        name: &'static str,
        tpe: TypeInfo,
        store_fn: Arc<dyn Fn(&Arc<Storage>, &E, K) -> Result<(), AppError> + Send + Sync>,
        load_fn: Arc<dyn Fn(&Arc<Storage>, K) -> Result<Option<Box<dyn FnOnce(E) -> E + Send>>, AppError> + Send + Sync>,
    ) -> Self {
        let factory = RuntimeFactory {
            tpe,
            kind: RuntimeKind::Relationship,
            build: Box::new(move |_props: ColumnProps| {
                Box::new(RelationshipRuntime::new(name, tpe, store_fn.clone(), load_fn.clone())) as Box<dyn ColumnRuntime<E, K>>
            }),
        };
        self.registry.insert(name, factory);
        self
    }

    pub fn build(self, cols: &[crate::schema::ColumnDef]) -> Result<ManualEntityRuntime<E, K>, crate::schema::SchemaError> {
        let mut runtimes = self.runtimes;
        for col in cols {
            match col {
                crate::schema::ColumnDef::Plain(field, idx, _, _) => {
                    if let Some(rt) = runtimes.iter().find(|r| r.name() == field.name) {
                        if rt.type_info() != field.ty {
                            return Err(crate::schema::SchemaError::TypeMismatch { field: field.name, expected: field.ty.name, actual: rt.type_info().name });
                        }
                        let expected_kind = match idx {
                            crate::schema::IndexingType::Off(_) => RuntimeKind::Plain,
                            crate::schema::IndexingType::Index(_) | crate::schema::IndexingType::Range(_) => RuntimeKind::Index,
                            crate::schema::IndexingType::Dict(_) => RuntimeKind::Dict,
                        };
                        if rt.kind() != expected_kind {
                            return Err(crate::schema::SchemaError::KindMismatch { field: field.name });
                        }
                        continue;
                    }
                    if let Some(factory) = self.registry.get(field.name) {
                        let expected_kind = match idx {
                            crate::schema::IndexingType::Off(p) => (RuntimeKind::Plain, *p),
                            crate::schema::IndexingType::Index(p) => (RuntimeKind::Index, *p),
                            crate::schema::IndexingType::Range(p) => (RuntimeKind::Index, *p),
                            crate::schema::IndexingType::Dict(p) => (RuntimeKind::Dict, *p),
                        };
                        if factory.kind != expected_kind.0 {
                            return Err(crate::schema::SchemaError::KindMismatch { field: field.name });
                        }
                        if factory.tpe != field.ty {
                            return Err(crate::schema::SchemaError::TypeMismatch { field: field.name, expected: field.ty.name, actual: factory.tpe.name });
                        }
                        runtimes.push((factory.build)(expected_kind.1));
                        continue;
                    }
                    return Err(crate::schema::SchemaError::MissingDependency(field.name));
                }
                crate::schema::ColumnDef::Relationship(field, _, _, _) => {
                    if let Some(rt) = runtimes.iter().find(|r| r.name() == field.name) {
                        if rt.type_info() != field.ty {
                            return Err(crate::schema::SchemaError::TypeMismatch { field: field.name, expected: field.ty.name, actual: rt.type_info().name });
                        }
                        if rt.kind() != RuntimeKind::Relationship
                            && rt.kind() != RuntimeKind::OneToOneCascade
                            && rt.kind() != RuntimeKind::OneToManyCascade
                            && rt.kind() != RuntimeKind::WriteFrom {
                            return Err(crate::schema::SchemaError::KindMismatch { field: field.name });
                        }
                        continue;
                    }
                    if let Some(factory) = self.registry.get(field.name) {
                        if factory.kind != RuntimeKind::Relationship
                            && factory.kind != RuntimeKind::OneToOneCascade
                            && factory.kind != RuntimeKind::OneToManyCascade
                            && factory.kind != RuntimeKind::WriteFrom {
                            return Err(crate::schema::SchemaError::KindMismatch { field: field.name });
                        }
                        if factory.tpe != field.ty {
                            return Err(crate::schema::SchemaError::TypeMismatch { field: field.name, expected: field.ty.name, actual: factory.tpe.name });
                        }
                        let props = ColumnProps::new(1, 0, 0);
                        runtimes.push((factory.build)(props));
                        continue;
                    }
                    return Err(crate::schema::SchemaError::MissingDependency(field.name));
                }
                crate::schema::ColumnDef::Transient(_) | crate::schema::ColumnDef::TransientRel(_, _) => {}
                crate::schema::ColumnDef::Key(_) => {}
            }
        }
        Ok(ManualEntityRuntime::new(
            self.name,
            self.pk_binding,
            self.pk_getter,
            self.seed_with_key,
            runtimes,
        ))
    }
}

/// Convenience: build a runtime from accessors and schema using the standard builder.
pub fn build_runtime_from_accessors<E, K>(
    name: &'static str,
    pk_binding: PlainTableBinding<K, ()>,
    pk_name: &'static str,
    pk_getter: fn(&E) -> K,
    seed_with_key: fn(&K) -> E,
    accessors: Vec<Accessor<E, K>>,
    schema: Vec<ColumnDef>,
) -> Result<std::sync::Arc<ManualEntityRuntime<E, K>>, AppError>
where
    E: 'static + Clone,
    K: crate::DbKey + Copy + Send + Sync + 'static,
{
    let builder = ManualRuntimeBuilder::new(name, pk_binding, pk_name, pk_getter, seed_with_key);
    let runtime = builder.with_accessors(accessors).build(&schema).map_err(|e| AppError::Custom(e.to_string()))?;
    Ok(std::sync::Arc::new(runtime))
}

impl ManualTestScope {
    /// Builds a fresh storage owner and view for provided DbDefs under a temp dir.
    pub async fn temp_storage(db_defs: Vec<DbDef>) -> redb::Result<(StorageOwner, Arc<Storage>, std::path::PathBuf), AppError> {
        let path = env::temp_dir().join(format!("redbit_manual_{}", rand::random::<u64>()));
        if path.exists() {
            let _ = fs::remove_dir_all(&path);
        }
        let (_, owner, storage) = StorageOwner::init(path.clone(), db_defs, 1, false).await?;
        Ok((owner, storage, path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{ColumnDef, ColumnProps, FieldDef, IndexingType, Multiplicity, SchemaError, TypeInfo};
    use crate::{impl_cachekey_integer, impl_redb_newtype_integer};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug, PartialEq)]
    struct Simple {
        id: u32,
        value: u64,
    }

    impl Simple {
        fn seed(id: &u32) -> Self {
            Simple { id: *id, value: 0 }
        }
    }

    fn pk_binding() -> PlainTableBinding<u32, ()> {
        PlainTableBinding::pk("Simple", "id", ColumnProps::for_key(0), true)
    }

    fn columns() -> Vec<Box<dyn ColumnRuntime<Simple, u32>>> {
        vec![
            Box::new(PlainColumnRuntime::new(
                "Simple",
                "value",
                "id",
                ColumnProps::new(1, 0, 0),
                |e: &Simple| e.value,
                |mut e: Simple, v| {
                    e.value = v;
                    e
                },
            )),
        ]
    }

    #[tokio::test]
    async fn manual_plain_roundtrip() -> Result<(), AppError> {
        let runtime = ManualEntityRuntime::new("Simple", pk_binding(), |e: &Simple| e.id, Simple::seed, columns());
        let db_defs = runtime.db_defs();
        let (owner, storage, path) = ManualTestScope::temp_storage(db_defs).await?;
        let simple = Simple { id: 7, value: 99 };
        runtime.store(&storage, &simple)?;
        let loaded = runtime.compose(&storage, 7)?.expect("entity should exist");
        assert_eq!(loaded, simple);
        owner.assert_last_refs();
        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    #[tokio::test]
    async fn compose_missing_returns_none() -> Result<(), AppError> {
        let runtime = ManualEntityRuntime::new("Simple", pk_binding(), |e: &Simple| e.id, Simple::seed, columns());
        let db_defs = runtime.db_defs();
        let (owner, storage, path) = ManualTestScope::temp_storage(db_defs).await?;
        let maybe = runtime.compose(&storage, 123)?;
        assert!(maybe.is_none());
        owner.assert_last_refs();
        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    #[tokio::test]
    async fn builder_missing_runtime_errors() {
        let cols_schema = vec![
            ColumnDef::Plain(FieldDef { name: "value", ty: TypeInfo::of::<u64>() }, IndexingType::Dict(ColumnProps::new(1, 0, 0)), None, false),
        ];
        let builder = ManualRuntimeBuilder::new("Simple", pk_binding(), "id", |e: &Simple| e.id, Simple::seed);
        let res = builder.build(&cols_schema);
        assert!(matches!(res, Err(SchemaError::MissingDependency("value"))));
    }

    #[tokio::test]
    async fn dict_roundtrip_and_dedup() -> Result<(), AppError> {
        #[derive(Copy, Clone, Debug, PartialEq, Eq)]
        struct DictVal(pub u32);
        impl_redb_newtype_integer!(DictVal, u32);
        impl_cachekey_integer!(DictVal, u32);

        #[derive(Clone, Debug, PartialEq, Eq)]
        struct DictEntity { id: u32, addr: DictVal }
        impl DictEntity { fn seed(id: &u32) -> Self { DictEntity { id: *id, addr: DictVal(0) } } }

        let dict_runtime: Box<dyn ColumnRuntime<DictEntity, u32>> = Box::new(DictColumnRuntime::new(
            "DictEntity",
            "addr",
            "id",
            ColumnProps::new(1, 0, 0),
            |e: &DictEntity| e.addr.clone(),
            |mut e: DictEntity, v| { e.addr = v; e },
        ));
        let runtime = ManualEntityRuntime::new(
            "DictEntity",
            PlainTableBinding::pk("DictEntity", "id", ColumnProps::for_key(0), true),
            |e: &DictEntity| e.id,
            DictEntity::seed,
            vec![dict_runtime],
        );
        let db_defs = runtime.db_defs();
        let (owner, storage, path) = ManualTestScope::temp_storage(db_defs).await?;

        let a1 = DictVal(7);
        let a2 = DictVal(7); // same value to test dedup paths
        let e1 = DictEntity { id: 1, addr: a1 };
        let e2 = DictEntity { id: 2, addr: a2 };
        runtime.store(&storage, &e1)?;
        runtime.store(&storage, &e2)?;

        let loaded1 = runtime.compose(&storage, 1)?.expect("e1");
        let loaded2 = runtime.compose(&storage, 2)?.expect("e2");
        assert_eq!(loaded1.addr.0, 7);
        assert_eq!(loaded2.addr.0, 7);
        owner.assert_last_refs();
        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    #[tokio::test]
    async fn builder_relationship_roundtrip() -> Result<(), AppError> {
        #[derive(Clone, Debug, PartialEq, Eq)]
        struct Child { id: u32, v: u8 }
        #[derive(Clone, Debug, PartialEq, Eq)]
        struct Parent { id: u32, child: Child }
        impl Parent { fn seed(id: &u32) -> Self { Parent { id: *id, child: Child { id: *id, v: 0 } } } }

        let rel_store = Arc::new(Mutex::new(HashMap::<u32, Child>::new()));
        let rel_load = rel_store.clone();

        let cols_schema = vec![
            ColumnDef::Relationship(
                FieldDef { name: "child", ty: TypeInfo::of::<Child>() },
                None,
                None,
                Multiplicity::OneToOne
            )
        ];

        let builder = ManualRuntimeBuilder::new("Parent", PlainTableBinding::pk("Parent", "id", ColumnProps::for_key(0), true), "id", |e: &Parent| e.id, Parent::seed)
            .register_relationship(
                "child",
                TypeInfo::of::<Child>(),
                Arc::new(move |_storage, parent: &Parent, _pk| {
                    rel_store.lock().unwrap().insert(parent.id, parent.child.clone());
                    Ok(())
                }),
                Arc::new(move |_storage, pk| {
                    let maybe = rel_load.lock().unwrap().get(&pk).cloned();
                    Ok(maybe.map(|c| Box::new(move |mut p: Parent| { p.child = c; p }) as Box<dyn FnOnce(Parent)->Parent + Send>))
                }),
            );

        let runtime = builder.build(&cols_schema).expect("relationship wired");
        let db_defs = runtime.db_defs();
        let (_owner, storage, path) = ManualTestScope::temp_storage(db_defs).await?;

        let p = Parent { id: 3, child: Child { id: 3, v: 9 } };
        runtime.store(&storage, &p)?;
        let loaded = runtime.compose(&storage, 3)?.expect("present");
        assert_eq!(loaded, p);
        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    #[tokio::test]
    async fn builder_validates_and_builds_runtime() -> Result<(), AppError> {
        let cols_schema = vec![
            ColumnDef::Plain(FieldDef { name: "value", ty: TypeInfo::of::<u64>() }, IndexingType::Off(ColumnProps::new(1, 0, 0)), None, false),
        ];
        let builder = ManualRuntimeBuilder::new("Simple", pk_binding(), "id", |e: &Simple| e.id, Simple::seed)
            .register_plain("value", |e: &Simple| e.value, |mut e: Simple, v| { e.value = v; e });
        let runtime = builder.build(&cols_schema).expect("builder should succeed");
        let db_defs = runtime.db_defs();
        let (_owner, storage, path) = ManualTestScope::temp_storage(db_defs).await?;
        let entity = Simple { id: 9, value: 123 };
        runtime.store(&storage, &entity)?;
        let loaded = runtime.compose(&storage, 9)?.expect("value present");
        assert_eq!(loaded.value, 123);
        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    #[tokio::test]
    async fn builder_uses_registry_for_index() -> Result<(), AppError> {
        let cols_schema = vec![
            ColumnDef::Plain(
                FieldDef { name: "secondary", ty: TypeInfo::of::<IdxVal>() },
                IndexingType::Index(ColumnProps::new(1, 0, 0)),
                None,
                false,
            )
        ];
        let builder = ManualRuntimeBuilder::new("Indexed", pk_binding_idx(), "id", |e: &Indexed| e.id, Indexed::seed)
            .register_index("secondary", |e: &Indexed| e.secondary, |mut e: Indexed, v| { e.secondary = v; e });
        let runtime = builder.build(&cols_schema).expect("builder should synthesize index runtime");
        let db_defs = runtime.db_defs();
        let (owner, storage, path) = ManualTestScope::temp_storage(db_defs).await?;
        let entity = Indexed { id: 11, secondary: IdxVal(88) };
        runtime.store(&storage, &entity)?;
        let loaded = runtime.compose(&storage, 11)?.expect("present");
        assert_eq!(loaded.secondary, IdxVal(88));
        owner.assert_last_refs();
        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    #[tokio::test]
    async fn builder_uses_registry_for_range_index() -> Result<(), AppError> {
        let cols_schema = vec![
            ColumnDef::Plain(
                FieldDef { name: "secondary", ty: TypeInfo::of::<IdxVal>() },
                IndexingType::Range(ColumnProps::new(1, 0, 0)),
                None,
                false,
            )
        ];
        let builder = ManualRuntimeBuilder::new("Indexed", pk_binding_idx(), "id", |e: &Indexed| e.id, Indexed::seed)
            .register_index("secondary", |e: &Indexed| e.secondary, |mut e: Indexed, v| { e.secondary = v; e });
        let runtime = builder.build(&cols_schema).expect("builder should synthesize range runtime");
        let db_defs = runtime.db_defs();
        let (owner, storage, path) = ManualTestScope::temp_storage(db_defs).await?;
        let entity = Indexed { id: 12, secondary: IdxVal(101) };
        runtime.store(&storage, &entity)?;
        let loaded = runtime.compose(&storage, 12)?.expect("present");
        assert_eq!(loaded.secondary, IdxVal(101));
        owner.assert_last_refs();
        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    struct IdxVal(pub u64);
    impl_redb_newtype_integer!(IdxVal, u64);
    impl_cachekey_integer!(IdxVal, u64);

    #[derive(Clone, Debug, PartialEq)]
    struct Indexed {
        id: u32,
        secondary: IdxVal,
    }

    impl Indexed {
        fn seed(id: &u32) -> Self {
            Indexed { id: *id, secondary: IdxVal(0) }
        }
    }

    fn pk_binding_idx() -> PlainTableBinding<u32, ()> {
        PlainTableBinding::pk("Indexed", "id", ColumnProps::for_key(0), true)
    }

    fn index_columns() -> Vec<Box<dyn ColumnRuntime<Indexed, u32>>> {
        vec![
            Box::new(IndexColumnRuntime::new(
                "Indexed",
                "secondary",
                ColumnProps::new(1, 0, 0),
                "id",
                |e: &Indexed| e.secondary,
                |mut e: Indexed, v| {
                    e.secondary = v;
                    e
                },
            )),
        ]
    }

    #[tokio::test]
    async fn manual_index_roundtrip() -> Result<(), AppError> {
        let runtime = ManualEntityRuntime::new("Indexed", pk_binding_idx(), |e: &Indexed| e.id, Indexed::seed, index_columns());
        let db_defs = runtime.db_defs();
        let (owner, storage, path) = ManualTestScope::temp_storage(db_defs).await?;
        let entity = Indexed { id: 5, secondary: IdxVal(42) };
        runtime.store(&storage, &entity)?;
        let loaded = runtime.compose(&storage, 5)?.expect("entity should exist");
        assert_eq!(loaded, entity);
        owner.assert_last_refs();
        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    #[tokio::test]
    async fn one_to_many_cascade_validates_pk_and_batches() -> Result<(), AppError> {
        #[derive(Clone, Debug, PartialEq)]
        struct Child { id: u32, v: u64 }
        #[derive(Clone, Debug, PartialEq)]
        struct Parent { id: u32, children: Vec<Child> }

        impl Child { fn seed(id: &u32) -> Self { Child { id: *id, v: 0 } } }
        impl Parent { fn seed(id: &u32) -> Self { Parent { id: *id, children: Vec::new() } } }

        let child_runtime = Arc::new(ManualEntityRuntime::new(
            "Child",
            PlainTableBinding::pk("Child", "id", ColumnProps::for_key(0), false),
            |c: &Child| c.id,
            Child::seed,
            vec![Box::new(PlainColumnRuntime::new(
                "Child",
                "v",
                "id",
                ColumnProps::new(1, 0, 0),
                |c: &Child| c.v,
                |mut c: Child, v| { c.v = v; c },
            ))],
        ));

        let cascade: Box<dyn ColumnRuntime<Parent, u32>> = Box::new(OneToManyCascadeRuntime::new(
            "children",
            Arc::clone(&child_runtime),
            Arc::new(|pk: u32, idx: usize| pk * 10 + idx as u32),
            |p: &Parent| p.children.clone(),
            |mut p: Parent, v| { p.children = v; p },
        ));

        let parent_runtime = ManualEntityRuntime::new(
            "Parent",
            PlainTableBinding::pk("Parent", "id", ColumnProps::for_key(0), true),
            |p: &Parent| p.id,
            Parent::seed,
            vec![cascade],
        );

        let db_defs = parent_runtime.db_defs();
        let (owner, storage, path) = ManualTestScope::temp_storage(db_defs).await?;
        let parents = vec![
            Parent { id: 1, children: vec![Child { id: 10, v: 1 }, Child { id: 11, v: 2 }] },
            Parent { id: 2, children: vec![Child { id: 20, v: 3 }] },
        ];
        parent_runtime.store_batch(&storage, &parents)?;
        let roundtrip = parent_runtime.compose(&storage, 1)?.unwrap();
        assert_eq!(roundtrip.children.len(), 2);
        let second = parent_runtime.compose(&storage, 2)?.unwrap();
        assert_eq!(second.children.len(), 1);
        owner.assert_last_refs();
        let _ = fs::remove_dir_all(path);
        Ok(())
    }

    #[tokio::test]
    async fn one_to_many_cascade_rejects_pk_mismatch() {
        #[derive(Clone, Debug, PartialEq)]
        struct Child { id: u32, v: u64 }
        #[derive(Clone, Debug, PartialEq)]
        struct Parent { id: u32, children: Vec<Child> }

        impl Child { fn seed(id: &u32) -> Self { Child { id: *id, v: 0 } } }
        impl Parent { fn seed(id: &u32) -> Self { Parent { id: *id, children: Vec::new() } } }

        let child_runtime = Arc::new(ManualEntityRuntime::new(
            "Child",
            PlainTableBinding::pk("Child", "id", ColumnProps::for_key(0), false),
            |c: &Child| c.id,
            Child::seed,
            vec![Box::new(PlainColumnRuntime::new(
                "Child",
                "v",
                "id",
                ColumnProps::new(1, 0, 0),
                |c: &Child| c.v,
                |mut c: Child, v| { c.v = v; c },
            ))],
        ));

        let cascade: Box<dyn ColumnRuntime<Parent, u32>> = Box::new(OneToManyCascadeRuntime::new(
            "children",
            Arc::clone(&child_runtime),
            Arc::new(|pk: u32, idx: usize| pk * 10 + idx as u32),
            |p: &Parent| p.children.clone(),
            |mut p: Parent, v| { p.children = v; p },
        ));

        let parent_runtime = ManualEntityRuntime::new(
            "Parent",
            PlainTableBinding::pk("Parent", "id", ColumnProps::for_key(0), true),
            |p: &Parent| p.id,
            Parent::seed,
            vec![cascade],
        );

        let db_defs = parent_runtime.db_defs();
        let (_owner, storage, path) = ManualTestScope::temp_storage(db_defs).await.unwrap();
        let bad = Parent { id: 1, children: vec![Child { id: 99, v: 1 }] };
        let err = parent_runtime.store(&storage, &bad).unwrap_err();
        assert!(format!("{err}").contains("pk mismatch"));
        let _ = fs::remove_dir_all(path);
    }
}
/// Binds a dictionary (value<->dict_pk<->entity pk) to storage.
#[allow(dead_code)]
pub struct DictTableBinding<K: crate::DbKey + Send + Sync, V: CacheKey + Send + Sync + Clone> {
    pub db_name: String,
    column_props: ColumnProps,
    redbit_def: RedbitTableDefinition<K, V, BytesPartitioner, Xxh3Partitioner, DictFactory<K, V>>,
}

impl<K: crate::DbKey + Send + Sync, V: CacheKey + Send + Sync + Clone> DictTableBinding<K, V> {
    pub fn new(entity: &str, column: &str, pk_name: &str, column_props: ColumnProps) -> Self {
        let base = format!("{}_{}_DICT", entity.to_uppercase(), column.to_uppercase());
        let dict_pk_to_ids_table = format!("{}_{}_DICT_INDEX", entity.to_uppercase(), column.to_uppercase());
        let value_by_dict_pk_table = format!("{}_{}_BY_DICT_PK", entity.to_uppercase(), column.to_uppercase());
        let value_to_dict_pk_table = format!("{}_{}_TO_DICT_PK", entity.to_uppercase(), column.to_uppercase());
        let dict_pk_by_pk_table = format!("{}_{}_DICT_PK_BY_{}", entity.to_uppercase(), column.to_uppercase(), pk_name.to_uppercase());
        let db_name = base.to_lowercase();
        let pk_by_ids_def: MultimapTableDefinition<'static, K, K> = MultimapTableDefinition::new(Box::leak(dict_pk_to_ids_table.into_boxed_str()));
        let value_by_dict_pk_def: TableDefinition<'static, K, V> = TableDefinition::new(Box::leak(value_by_dict_pk_table.into_boxed_str()));
        let value_to_dict_pk_def: TableDefinition<'static, V, K> = TableDefinition::new(Box::leak(value_to_dict_pk_table.into_boxed_str()));
        let dict_pk_by_pk_def: TableDefinition<'static, K, K> = TableDefinition::new(Box::leak(dict_pk_by_pk_table.into_boxed_str()));

        let redbit_def = RedbitTableDefinition::new(
            false,
            Partitioning::by_value(column_props.shards),
            DictFactory::new(&db_name, column_props.lru_cache_size, pk_by_ids_def, value_by_dict_pk_def, value_to_dict_pk_def, dict_pk_by_pk_def),
        );
        Self { db_name, column_props, redbit_def }
    }

    pub fn db_def(&self) -> DbDef {
        DbDef {
            name: self.db_name.clone(),
            shards: self.column_props.shards,
            db_cache_weight_or_zero: self.column_props.db_cache_weight,
            lru_cache_size_or_zero: self.column_props.lru_cache_size,
        }
    }

    pub fn writer(&self, storage: &Arc<Storage>) -> redb::Result<ShardedTableWriter<K, V, BytesPartitioner, Xxh3Partitioner, DictFactory<K, V>>, AppError> {
        self.redbit_def.to_write_field(storage)
    }

    pub fn reader(&self, storage: &Arc<Storage>) -> redb::Result<ShardedTableReader<K, V, BytesPartitioner, Xxh3Partitioner>, AppError> {
        self.redbit_def.to_read_field(storage)
    }

    pub fn ensure_table(&self, storage: &Arc<Storage>) -> Result<(), AppError> {
        let writer = self.writer(storage)?;
        writer.begin(Durability::None)?;
        writer.flush()?;
        writer.shutdown()?;
        Ok(())
    }
}

pub struct DictColumnRuntime<
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
> where for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static {
    binding: DictTableBinding<K, V>,
    getter: fn(&E) -> V,
    setter: fn(E, V) -> E,
    name: &'static str,
    tpe: TypeInfo,
}


pub struct RelationshipRuntime<
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
> {
    name: &'static str,
    tpe: TypeInfo,
    store_fn: Arc<dyn Fn(&Arc<Storage>, &E, K) -> Result<(), AppError> + Send + Sync>,
    load_fn: Arc<dyn Fn(&Arc<Storage>, K) -> Result<Option<Box<dyn FnOnce(E) -> E + Send>>, AppError> + Send + Sync>,
}

impl<
    E: 'static,
    K: crate::DbKey + Copy + Send + Sync + 'static,
> RelationshipRuntime<E, K> {
    pub fn new(
        name: &'static str,
        tpe: TypeInfo,
        store_fn: Arc<dyn Fn(&Arc<Storage>, &E, K) -> Result<(), AppError> + Send + Sync>,
        load_fn: Arc<dyn Fn(&Arc<Storage>, K) -> Result<Option<Box<dyn FnOnce(E) -> E + Send>>, AppError> + Send + Sync>,
    ) -> Self {
        Self { name, tpe, store_fn, load_fn }
    }
}

impl<
    E: 'static + Clone,
    K: crate::DbKey + Copy + Send + Sync + 'static,
> ColumnRuntime<E, K> for RelationshipRuntime<E, K> {
    fn name(&self) -> &'static str { self.name }
    fn column_name(&self) -> Option<&'static str> { Some(self.name) }
    fn type_info(&self) -> TypeInfo { self.tpe }
    fn kind(&self) -> RuntimeKind { RuntimeKind::Relationship }
    fn db_defs(&self) -> Vec<DbDef> { Vec::new() }
    fn ensure_table(&self, _storage: &Arc<Storage>) -> Result<(), AppError> { Ok(()) }
    fn store(&self, storage: &Arc<Storage>, entity: &E, pk: K) -> Result<(), AppError> {
        (self.store_fn)(storage, entity, pk)
    }
    fn load(&self, storage: &Arc<Storage>, pk: K) -> Result<LoadResult<E>, AppError> {
        match (self.load_fn)(storage, pk)? {
            Some(apply) => Ok(LoadResult::Value(apply)),
            None => Ok(LoadResult::Reject),
        }
    }
}

/// Write-from runtime that runs a hook against existing writer batches.
pub struct WriteFromRuntime<
    P: 'static + Clone,
    PK: crate::DbKey + Copy + Send + Sync + 'static,
> {
    name: &'static str,
    tpe: TypeInfo,
    apply: Arc<dyn Fn(&RuntimeWriters<P, PK>, Option<&dyn ChildWriters>, &[(PK, P)]) -> Result<(), AppError> + Send + Sync>,
}

impl<P: 'static + Clone, PK: crate::DbKey + Copy + Send + Sync + 'static> WriteFromRuntime<P, PK> {
    pub fn new(
        name: &'static str,
        tpe: TypeInfo,
        apply: Arc<dyn Fn(&RuntimeWriters<P, PK>, Option<&dyn ChildWriters>, &[(PK, P)]) -> Result<(), AppError> + Send + Sync>,
    ) -> Self {
        Self { name, tpe, apply }
    }
}

impl<P: 'static + Clone, PK: crate::DbKey + Copy + Send + Sync + 'static> ColumnRuntime<P, PK> for WriteFromRuntime<P, PK> {
    fn name(&self) -> &'static str { self.name }
    fn column_name(&self) -> Option<&'static str> { Some(self.name) }
    fn type_info(&self) -> TypeInfo { self.tpe }
    fn kind(&self) -> RuntimeKind { RuntimeKind::WriteFrom }
    fn db_defs(&self) -> Vec<DbDef> { Vec::new() }
    fn ensure_table(&self, _storage: &Arc<Storage>) -> Result<(), AppError> { Ok(()) }
    fn store(&self, storage: &Arc<Storage>, entity: &P, pk: PK) -> Result<(), AppError> {
        // No-op when parent writers are not provided (manual store paths skip write_from).
        let _ = storage;
        let _ = entity;
        let _ = pk;
        Ok(())
    }
    fn store_many(&self, storage: &Arc<Storage>, items: &[(PK, P)]) -> Result<(), AppError> {
        let _ = storage;
        let _ = items;
        Ok(())
    }
    fn store_many_with_children(&self, _storage: &Arc<Storage>, items: &[(PK, P)], child_writers: Option<&dyn ChildWriters>, parent_writers: Option<&RuntimeWriters<P, PK>>) -> Result<(), AppError> {
        let parent = match parent_writers {
            Some(p) => p,
            None => return Ok(()), // nothing to do without parent writers (outside batch contexts)
        };
        // If child writers are unavailable, skip rather than failing hard.
        if child_writers.is_none() {
            return Ok(());
        }
        (self.apply)(parent, child_writers, items)
    }
    fn begin_batch(&self, _storage: &Arc<Storage>) -> Result<Option<Box<dyn ColumnBatch<P, PK>>>, AppError> {
        Ok(None)
    }
    fn load(&self, _storage: &Arc<Storage>, _pk: PK) -> Result<LoadResult<P>, AppError> {
        Ok(LoadResult::Reject)
    }
}

/// Cascading one-to-one / option relationship that delegates to a child runtime.
pub struct OneToOneCascadeRuntime<
    P: 'static + Clone,
    C: 'static + Clone + Send + Sync,
    PK: crate::DbKey + Copy + Send + Sync + 'static,
    CK: crate::DbKey + Copy + Send + Sync + 'static + PartialEq,
> {
    name: &'static str,
    tpe: TypeInfo,
    child_runtime: Arc<ManualEntityRuntime<C, CK>>,
    parent_pk_to_child_pk: fn(PK) -> CK,
    getter: fn(&P) -> Option<C>,
    setter: fn(P, Option<C>) -> P,
}

impl<P: 'static + Clone, C: 'static + Clone + Send + Sync, PK: crate::DbKey + Copy + Send + Sync + 'static, CK: crate::DbKey + Copy + Send + Sync + 'static + PartialEq>
OneToOneCascadeRuntime<P, C, PK, CK> {
    pub fn new(
        name: &'static str,
        child_runtime: Arc<ManualEntityRuntime<C, CK>>,
        parent_pk_to_child_pk: fn(PK) -> CK,
        getter: fn(&P) -> Option<C>,
        setter: fn(P, Option<C>) -> P,
    ) -> Self {
        Self {
            name,
            tpe: TypeInfo::of::<C>(),
            child_runtime,
            parent_pk_to_child_pk,
            getter,
            setter,
        }
    }
}

impl<P: 'static + Clone, C: 'static + Clone + Send + Sync, PK: crate::DbKey + Copy + Send + Sync + 'static, CK: crate::DbKey + Copy + Send + Sync + 'static + PartialEq>
ColumnRuntime<P, PK> for OneToOneCascadeRuntime<P, C, PK, CK> {
    fn name(&self) -> &'static str { self.name }
    fn column_name(&self) -> Option<&'static str> { Some(self.name) }
    fn type_info(&self) -> TypeInfo { self.tpe }
    fn kind(&self) -> RuntimeKind { RuntimeKind::OneToOneCascade }
    fn db_defs(&self) -> Vec<DbDef> { self.child_runtime.db_defs() }
    fn ensure_table(&self, storage: &Arc<Storage>) -> Result<(), AppError> {
        self.child_runtime.pk_binding.ensure_table(storage)?;
        for c in &self.child_runtime.columns {
            c.ensure_table(storage)?;
        }
        Ok(())
    }
    fn store(&self, storage: &Arc<Storage>, entity: &P, pk: PK) -> Result<(), AppError> {
        if let Some(child) = (self.getter)(entity) {
            let child_pk = (self.parent_pk_to_child_pk)(pk);
            let expected = (self.child_runtime.pk_getter)(&child);
            if child_pk != expected {
                return Err(AppError::Custom("cascade child pk mismatch".into()));
            }
            self.child_runtime.store_batch(storage, &[child])?;
        }
        Ok(())
    }
    fn store_many(&self, storage: &Arc<Storage>, items: &[(PK, P)]) -> Result<(), AppError> {
        self.store_many_with_children(storage, items, None, None)
    }
    fn store_many_with_children(&self, storage: &Arc<Storage>, items: &[(PK, P)], child_writers: Option<&dyn ChildWriters>, _parent_writers: Option<&RuntimeWriters<P, PK>>) -> Result<(), AppError> {
        if items.is_empty() { return Ok(()); }
        let mut children: Vec<C> = Vec::new();
        for (pk, parent) in items {
            if let Some(child) = (self.getter)(parent) {
                let child_pk = (self.parent_pk_to_child_pk)(*pk);
                let expected = (self.child_runtime.pk_getter)(&child);
                if child_pk != expected {
                    return Err(AppError::Custom("cascade child pk mismatch".into()));
                }
                children.push(child);
            }
        }
        if children.is_empty() { return Ok(()); }
        if let Some(child_writers) = child_writers {
            if let Some(tree) = child_writers.as_any().downcast_ref::<RuntimeWritersWithChildren<C, CK>>() {
                return self.child_runtime.store_batch_with_writer_tree(storage, tree, &children);
            }
            if let Some(flat) = child_writers.as_any().downcast_ref::<RuntimeWriters<C, CK>>() {
                return self.child_runtime.store_batch_with_writers(storage, flat, &children);
            }
        }
        self.child_runtime.store_batch(storage, &children)
    }
    fn begin_batch(&self, _storage: &Arc<Storage>) -> Result<Option<Box<dyn ColumnBatch<P, PK>>>, AppError> {
        Ok(None)
    }
    fn load(&self, storage: &Arc<Storage>, pk: PK) -> Result<LoadResult<P>, AppError> {
        let setter = self.setter;
        let child_pk = (self.parent_pk_to_child_pk)(pk);
        match self.child_runtime.compose(storage, child_pk)? {
            Some(child) => Ok(LoadResult::Value(Box::new(move |mut p: P| {
                p = setter(p, Some(child));
                p
            }))),
            None => Ok(LoadResult::Value(Box::new(move |mut p: P| {
                p = setter(p, None);
                p
            }))),
        }
    }
}

/// Cascading one-to-many relationship that delegates to a child runtime.
pub struct OneToManyCascadeRuntime<
    P: 'static + Clone,
    C: 'static + Clone + Send + Sync,
    PK: crate::DbKey + Copy + Send + Sync + 'static,
    CK: crate::DbKey + Copy + Send + Sync + 'static + PartialEq,
> {
    name: &'static str,
    tpe: TypeInfo,
    child_runtime: Arc<ManualEntityRuntime<C, CK>>,
    child_pk_at: Arc<dyn Fn(PK, usize) -> CK + Send + Sync>,
    getter: fn(&P) -> Vec<C>,
    setter: fn(P, Vec<C>) -> P,
}

impl<P: 'static + Clone, C: 'static + Clone + Send + Sync, PK: crate::DbKey + Copy + Send + Sync + 'static, CK: crate::DbKey + Copy + Send + Sync + 'static + PartialEq>
OneToManyCascadeRuntime<P, C, PK, CK> {
    pub fn new(
        name: &'static str,
        child_runtime: Arc<ManualEntityRuntime<C, CK>>,
        child_pk_at: Arc<dyn Fn(PK, usize) -> CK + Send + Sync>,
        getter: fn(&P) -> Vec<C>,
        setter: fn(P, Vec<C>) -> P,
    ) -> Self {
        Self {
            name,
            tpe: TypeInfo::of::<C>(),
            child_runtime,
            child_pk_at,
            getter,
            setter,
        }
    }
}

impl<P: 'static + Clone, C: 'static + Clone + Send + Sync, PK: crate::DbKey + Copy + Send + Sync + 'static, CK: crate::DbKey + Copy + Send + Sync + 'static + PartialEq> ColumnRuntime<P, PK> for OneToManyCascadeRuntime<P, C, PK, CK> {
    fn name(&self) -> &'static str { self.name }
    fn column_name(&self) -> Option<&'static str> { Some(self.name) }
    fn type_info(&self) -> TypeInfo { self.tpe }
    fn kind(&self) -> RuntimeKind { RuntimeKind::OneToManyCascade }
    fn db_defs(&self) -> Vec<DbDef> { self.child_runtime.db_defs() }
    fn ensure_table(&self, storage: &Arc<Storage>) -> Result<(), AppError> {
        self.child_runtime.pk_binding.ensure_table(storage)?;
        for c in &self.child_runtime.columns {
            c.ensure_table(storage)?;
        }
        Ok(())
    }
    fn store(&self, storage: &Arc<Storage>, entity: &P, pk: PK) -> Result<(), AppError> {
        let children = (self.getter)(entity);
        if children.is_empty() {
            return Ok(());
        }
        let mut patched_children: Vec<C> = Vec::with_capacity(children.len());
        for (idx, child) in children.into_iter().enumerate() {
            let expected = (self.child_pk_at)(pk, idx);
            let actual = (self.child_runtime.pk_getter)(&child);
            if actual != expected {
                return Err(AppError::Custom("cascade child pk mismatch".into()));
            }
            patched_children.push(child);
        }
        self.child_runtime.store_batch(storage, &patched_children)?;
        Ok(())
    }
    fn store_many(&self, storage: &Arc<Storage>, items: &[(PK, P)]) -> Result<(), AppError> {
        self.store_many_with_children(storage, items, None, None)
    }
    fn store_many_with_children(&self, storage: &Arc<Storage>, items: &[(PK, P)], child_writers: Option<&dyn ChildWriters>, _parent_writers: Option<&RuntimeWriters<P, PK>>) -> Result<(), AppError> {
        if items.is_empty() { return Ok(()); }
        let mut all_children: Vec<C> = Vec::new();
        for (pk, parent) in items {
            let children = (self.getter)(parent);
            for (idx, child) in children.into_iter().enumerate() {
                let expected = (self.child_pk_at)(*pk, idx);
                let actual = (self.child_runtime.pk_getter)(&child);
                if actual != expected {
                    return Err(AppError::Custom("cascade child pk mismatch".into()));
                }
                all_children.push(child);
            }
        }
        if all_children.is_empty() { return Ok(()); }
        if let Some(child_writers) = child_writers {
            if let Some(tree) = child_writers.as_any().downcast_ref::<RuntimeWritersWithChildren<C, CK>>() {
                return self.child_runtime.store_batch_with_writer_tree(storage, tree, &all_children);
            }
            if let Some(flat) = child_writers.as_any().downcast_ref::<RuntimeWriters<C, CK>>() {
                return self.child_runtime.store_batch_with_writers(storage, flat, &all_children);
            }
        }
        self.child_runtime.store_batch(storage, &all_children)
    }
    fn load(&self, storage: &Arc<Storage>, pk: PK) -> Result<LoadResult<P>, AppError> {
        let child_pk_at = self.child_pk_at.clone();
        let mut out: Vec<C> = Vec::new();
        let mut idx = 0usize;
        loop {
            let child_pk = (child_pk_at)(pk, idx);
            match self.child_runtime.compose(storage, child_pk)? {
                Some(child) => out.push(child),
                None => break,
            }
            idx += 1;
            if idx > 10_000 { break; } // guard runaway
        }
        let setter = self.setter;
        Ok(LoadResult::Value(Box::new(move |mut p: P| {
            p = setter(p, out);
            p
        })))
    }
}

impl<
    E: 'static + Clone,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
> DictColumnRuntime<E, K, V>
where
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    pub fn new(
        entity: &str,
        column: &'static str,
        pk_name: &str,
        column_props: ColumnProps,
        getter: fn(&E) -> V,
        setter: fn(E, V) -> E,
    ) -> Self {
        let binding = DictTableBinding::new(entity, column, pk_name, column_props);
        Self { binding, getter, setter, name: column, tpe: TypeInfo::of::<V>() }
    }
}

impl<
    E: 'static + Clone,
    K: crate::DbKey + Copy + Send + Sync + 'static,
    V: CacheKey + Clone + Send + Sync + 'static,
> ColumnRuntime<E, K> for DictColumnRuntime<E, K, V>
where
    for<'a> <V as redb::Value>::SelfType<'a>: Into<V> + Clone + 'static,
{
    fn name(&self) -> &'static str { self.name }
    fn column_name(&self) -> Option<&'static str> { Some(self.name) }
    fn type_info(&self) -> TypeInfo { self.tpe }
    fn kind(&self) -> RuntimeKind { RuntimeKind::Dict }
    fn db_defs(&self) -> Vec<DbDef> { vec![self.binding.db_def()] }
    fn ensure_table(&self, storage: &Arc<Storage>) -> Result<(), AppError> { self.binding.ensure_table(storage) }
    fn store(&self, storage: &Arc<Storage>, entity: &E, pk: K) -> Result<(), AppError> {
        let val = (self.getter)(entity);
        let writer = self.binding.writer(storage)?;
        writer.begin(Durability::None)?;
        writer.insert_on_flush(pk, val)?;
        writer.flush()?;
        writer.shutdown()?;
        Ok(())
    }
    fn store_many(&self, storage: &Arc<Storage>, items: &[(K, E)]) -> Result<(), AppError> {
        if items.is_empty() { return Ok(()); }
        let writer = self.binding.writer(storage)?;
        writer.begin(Durability::None)?;
        for (pk, e) in items {
            let val = (self.getter)(e);
            writer.insert_on_flush(*pk, val)?;
        }
        writer.flush()?;
        writer.shutdown()?;
        Ok(())
    }

    fn store_many_fast(&self, storage: &Arc<Storage>, items: &[(K, E)], durability: Durability) -> Result<bool, AppError> {
        if items.is_empty() { return Ok(true); }
        let writer = self.binding.writer(storage)?;
        writer.begin(durability)?;
        for (pk, e) in items {
            let val = (self.getter)(e);
            writer.insert_on_flush(*pk, val)?;
        }
        writer.flush()?;
        writer.shutdown()?;
        Ok(true)
    }

    fn begin_batch(&self, storage: &Arc<Storage>) -> Result<Option<Box<dyn ColumnBatch<E, K>>>, AppError> {
        let writer = self.binding.writer(storage)?;
        Ok(Some(Box::new(DictBatch { writer, getter: self.getter })))
    }
    fn load(&self, storage: &Arc<Storage>, pk: K) -> Result<LoadResult<E>, AppError> {
        let setter = self.setter;
        let reader = self.binding.reader(storage)?;
        match reader.get_value(pk)? {
            Some(guard) => {
                let owned: V = guard.value().into();
                drop(guard);
                Ok(LoadResult::Value(Box::new(move |mut e: E| {
                    e = setter(e, owned);
                    e
                })))
            }
            None => Ok(LoadResult::Reject),
        }
    }
}
