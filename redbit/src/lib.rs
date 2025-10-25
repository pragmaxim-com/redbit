#![feature(test)]
extern crate test;

pub mod query;
pub mod utf8_serde_enc;
pub mod hex_serde_enc;
pub mod base64_serde_enc;
pub mod retry;
pub mod logger;
pub mod storage;

pub use axum;
pub use axum::body::Body;
pub use axum::extract;
pub use axum::http::StatusCode;
pub use axum::response::IntoResponse;
pub use axum::response::Response;
pub use axum_streams;
pub use bincode::{decode_from_slice, encode_to_vec, Decode, Encode};
pub use chrono;
pub use futures;
pub use futures::stream::{self, StreamExt};
pub use futures_util::stream::TryStreamExt;
pub use http;
pub use http::HeaderValue;
pub use inventory;
pub use lru::LruCache;
pub use macros::column;
pub use macros::entity;
pub use macros::pointer_key;
pub use macros::root_key;
pub use macros::Entity;
pub use macros::PointerKey;
pub use macros::RootKey;
pub use once_cell;
pub use query::*;
pub use indexmap::IndexMap;
pub use rand;
pub use redb;
pub use redb::{
    Database, Durability, Key, TypeName, Value, MultimapTable, MultimapTableDefinition, ReadOnlyMultimapTable, ReadOnlyTable, ReadTransaction,
    ReadableDatabase, ReadableMultimapTable, ReadableTable, ReadableTableMetadata, Table, TableDefinition, TableError, TableStats, TransactionError, WriteTransaction,
};
pub use serde;
pub use serde::Deserialize;
pub use serde::Deserializer;
pub use serde::Serialize;
pub use serde::Serializer;
pub use serde_json;
pub use serde_json::json;
pub use serde_urlencoded;
pub use serde_with;
pub use std::any::type_name;
use std::borrow::Borrow;
pub use std::cmp::Ordering;
pub use std::collections::HashMap;
pub use std::collections::VecDeque;
pub use std::fmt::Debug;
use std::hash::Hash;
pub use std::time::Instant;
pub use std::pin::Pin;
pub use std::sync::Arc;
pub use std::sync::Weak;
pub use std::thread;
pub use std::time::Duration;
pub use storage::context::{ReadTxContext, WriteTxContext};
pub use storage::partitioning::{Partitioning, BytesPartitioner, Xxh3Partitioner, KeyPartitioner, ValuePartitioner};
pub use storage::table_dict_read::ReadOnlyDictTable;
pub use storage::table_dict_read_sharded::ShardedReadOnlyDictTable;
pub use storage::table_dict_write::{DictTable, DictFactory};
pub use storage::table_index_read::ReadOnlyIndexTable;
pub use storage::table_index_read_sharded::ShardedReadOnlyIndexTable;
pub use storage::table_index_write::{IndexTable, IndexFactory};
pub use storage::table_plain_read::ReadOnlyPlainTable;
pub use storage::table_plain_read_sharded::ShardedReadOnlyPlainTable;
pub use storage::table_plain_write::PlainFactory;
pub use storage::table_writer_api::{StartFuture, StopFuture, TaskResult, FlushFuture, WriterLike, WriteTableLike};
pub use storage::async_boundary::CopyOwnedValue;
pub use storage::tx_fsm::TxFSM;
pub use storage::table_writer::ShardedTableWriter;
pub use storage::init::Storage;
pub use storage::init::StorageOwner;
pub use urlencoding;
pub use utoipa;
pub use utoipa::openapi;
pub use utoipa::IntoParams;
pub use utoipa::PartialSchema;
pub use utoipa::ToSchema;
pub use utoipa_axum;
pub use utoipa_axum::router::OpenApiRouter;
pub use utoipa_swagger_ui;

use crate::axum::extract::rejection::JsonRejection;
use crate::axum::extract::FromRequest;
use crate::axum::Router;
use crate::utoipa::OpenApi;
use crate::utoipa_swagger_ui::SwaggerUi;
use axum::body::Bytes;
use axum::extract::Request;
use crossbeam::channel::{RecvError, SendError};
use serde::de::DeserializeOwned;
use std::net::SocketAddr;
use std::ops::Add;
use std::sync::PoisonError;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinError;
use tower_http::cors::CorsLayer;

pub trait ColInnerType { type Repr; }

pub trait IndexedPointer: Copy {
    type Index: Copy + Ord + Add<Output = Self::Index> + Default;
    fn index(&self) -> Self::Index;
    fn next_index(&self) -> Self;
    fn nth_index(&self, n: usize) -> Self;
    fn rollback_or_init(&self, n: u32) -> Self;
}

pub trait RootPointer: IndexedPointer + Copy {
    fn is_pointer(&self) -> bool;
    fn from_many(indexes: &[Self::Index]) -> Vec<Self>;
}

pub trait ChildPointer: IndexedPointer + Copy {
    type Parent: IndexedPointer + Copy;
    fn is_pointer(&self) -> bool;
    fn parent(&self) -> Self::Parent;
    fn from_parent(parent: Self::Parent, index: Self::Index) -> Self;
}

pub trait ForeignKey<CH>: Copy
where
    CH: ChildPointer + Copy,
    CH::Parent: IndexedPointer + Copy,
{
    fn fk_range(&self) -> (CH, CH);
}

impl<CH> ForeignKey<CH> for CH::Parent
where
    CH: ChildPointer + Copy,
    CH::Parent: IndexedPointer + Copy,
{
    fn fk_range(&self) -> (CH, CH) {
        (CH::from_parent(*self, CH::Index::default()), CH::from_parent(self.next_index(), CH::Index::default()))
    }
}

pub trait Sampleable: Default + Sized + Clone {
    fn next_value(&self) -> Self;
    fn nth_value(&self, n: usize) -> Self {
        let mut value = self.clone(); // convert &Self → Self
        let mut n = n;
        while n > 0 {
            value = value.next_value();
            n -= 1;
        }
        value
    }
    fn sample_many(n: usize) -> Vec<Self> {
        let mut values = Vec::with_capacity(n);
        let mut value = Self::default();
        for _ in 0..n {
            values.push(value.clone());
            value = value.next_value();
        }
        values
    }
}
macro_rules! impl_sampleable_for_primitive {
    ($($t:ty),*) => {
        $(
            impl Sampleable for $t {
                fn next_value(&self) -> Self {
                    self.wrapping_add(1)
                }
            }
        )*
    };
}

impl_sampleable_for_primitive!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

#[macro_export]
macro_rules! impl_copy_owned_value_identity {
        ($t:ty) => {
            impl CopyOwnedValue for $t
            where
                <$t as redb::Value>::SelfType<'static>: Copy + Send + 'static,
            {
                type Unit = <$t as redb::Value>::SelfType<'static>;
                #[inline]
                fn to_unit<'a>(v: <$t as redb::Value>::SelfType<'a>) -> Self::Unit
                where
                    Self: 'a,
                {
                    v
                }
                #[inline]
                fn from_unit<'a>(u: Self::Unit) -> <$t as redb::Value>::SelfType<'a>
                where
                    Self: 'a,
                {
                    u
                }
                #[inline]
                fn to_unit_ref<'a>(v: &<$t as redb::Value>::SelfType<'a>) -> Self::Unit
                where
                    Self: 'a,
                {
                    *v
                }
                #[inline]
                fn as_value_from_unit<'a>(u: &'a Self::Unit) -> <$t as redb::Value>::SelfType<'a>
                where
                    Self: 'a,
                {
                    *u
                }
            }
        };
    }

pub trait UrlEncoded {
    fn url_encode(&self) -> String;
}

pub trait CacheKey: Key {
    type CK: Eq + Hash + Clone;
    fn cache_key<'a>(v: &Self::SelfType<'a>) -> Self::CK
    where
        Self: 'a;
}

pub trait BinaryCodec {
    fn from_le_bytes(bytes: &[u8]) -> Self;
    fn as_le_bytes(&self) -> Vec<u8>;
    fn size() -> usize;
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TableInfo {
    pub table_name: String,
    pub table_entries: u64,
    pub tree_height: u32,
    pub leaf_pages: u64,
    pub branch_pages: u64,
    pub stored_leaf_bytes: u64,
    pub metadata_bytes: u64,
    pub fragmented_bytes: u64,
}

impl TableInfo {
    pub fn from_stats(table_name: &str, table_entries: u64, stats: TableStats) -> Self {
        TableInfo {
            table_name: table_name.to_string(),
            table_entries,
            tree_height: stats.tree_height(),
            leaf_pages: stats.leaf_pages(),
            branch_pages: stats.branch_pages(),
            stored_leaf_bytes: stats.stored_bytes(),
            metadata_bytes: stats.metadata_bytes(),
            fragmented_bytes: stats.fragmented_bytes(),
        }
    }
}

pub trait ByteVecColumnSerde {
    fn decoded_example() -> Vec<u8>;
    fn encoded_example() -> String;
    fn next_value(value: &[u8]) -> Vec<u8> {
        let mut vec = value.to_owned();
        if let Some(last) = vec.last_mut() {
            *last = last.wrapping_add(1);
        } else {
            vec.push(1);
        }
        vec
    }
}

#[derive(Debug, Error)]
pub enum AppError {

    #[error("Database error: {0}")]
    Database(#[from] redb::DatabaseError),

    #[error("redb error: {0}")]
    Redb(#[from] redb::Error),

    #[error("redb transaction error: {0}")]
    RedbTransaction(#[from] redb::TransactionError),

    #[error("redb storage error: {0}")]
    RedbStorage(#[from] redb::StorageError),

    #[error("redb table error: {0}")]
    RedbTable(#[from] redb::TableError),

    #[error("redb commit error: {0}")]
    RedbCommit(#[from] redb::CommitError),

    #[error("serde error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] http::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Json rejection: {0}")]
    JsonRejection(#[from] JsonRejection),

    #[error("Join: {0}")]
    JoinError(#[from] JoinError),

    #[error("Recv: {0}")]
    RecvError(#[from] RecvError),

    #[error("Not Found: {0}")]
    NotFound(String),

    #[error("Bad Request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("Custom error: {0}")]
    Custom(String),
}

impl AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::NotFound(_)      => StatusCode::NOT_FOUND,
            AppError::BadRequest(_)    => StatusCode::BAD_REQUEST,
            AppError::JsonRejection(r) => r.status(),
            _                          => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl<T> From<SendError<T>> for AppError
{
    fn from(e: SendError<T>) -> Self {
        AppError::Custom(format!("send error: {:?}", e.to_string()))
    }
}

impl<T> From<PoisonError<T>> for AppError
{
    fn from(e: PoisonError<T>) -> Self {
        AppError::Custom(format!("Poison error: {:?}", e.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ParsePointerError {
    #[error("invalid pointer format")]
    Format,
    #[error("invalid integer: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
}

impl From<AppError> for axum::Error {
    fn from(val: AppError) -> Self {
        axum::Error::new(val.to_string())
    }
}

// Create our own JSON extractor by wrapping `axum::Json`. This makes it easy to override the
// rejection and provide our own which formats errors to match our application.
//
// `axum::Json` responds with plain text if the input is invalid.
#[derive(FromRequest, Deserialize)]
#[from_request(via(crate::axum::Json), rejection(AppError))]
pub struct AppJson<T>(pub T);

impl<T> IntoResponse for AppJson<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

#[derive(Deserialize)]
pub struct MaybeJson<T>(pub Option<T>);

impl<S, T> FromRequest<S> for MaybeJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let body_bytes = match Bytes::from_request(req, state).await {
            Ok(bytes) => bytes,
            Err(_) => return Ok(MaybeJson(None)),
        };

        if body_bytes.is_empty() {
            return Ok(MaybeJson(None));
        }

        let parsed = serde_json::from_slice::<T>(&body_bytes)?;

        Ok(MaybeJson(Some(parsed)))
    }
}

#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    pub message: String,
    pub code: u16,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let message = match self {
            AppError::JsonRejection(rej) => rej.body_text(),
            other                        => other.to_string(),
        };
        (status, AppJson(ErrorResponse { message, code: status.as_u16() })).into_response()
    }
}

#[derive(Clone)]
pub struct RequestState {
    pub storage: Arc<Storage>,
}

#[derive(Clone, Debug)]
pub struct DbDef { pub name: String, pub shards: usize, pub db_cache_weight_or_zero: usize, pub lru_cache_size_or_zero: usize }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbDefWithCache { pub name: String, pub shards: usize, pub db_cache_weight: usize, pub db_cache_in_mb: usize, pub lru_cache: usize }
impl DbDefWithCache {
    pub fn new(dn_def: DbDef, db_cache_in_mb: usize) -> Self {
        DbDefWithCache {
            name: dn_def.name.clone(),
            shards: dn_def.shards,
            db_cache_weight: dn_def.db_cache_weight_or_zero,
            lru_cache: dn_def.lru_cache_size_or_zero,
            db_cache_in_mb,
        }
    }
    pub fn validate(&self) -> Result<(), AppError> {
        if self.shards == 0 {
            Err(AppError::Custom(format!(
                "column `{}`: shards == 0 is not supported; use 1 (single) or >= 2 (sharded)", self.name
            )))
        } else {
            Ok(())
        }
    }
}


pub struct StructInfo {
    pub name: &'static str,
    pub root: bool,
    pub routes_fn: fn() -> OpenApiRouter<RequestState>,
    pub db_defs: fn() -> Vec<DbDef>,
}

inventory::collect!(StructInfo);

#[derive(OpenApi)]
#[openapi(info(license(name = "MIT")))]
pub struct ApiDoc;

#[derive(Debug, Clone, ToSchema, Serialize, Deserialize)]
pub enum FilterOp<T> {
    Eq(T),
    Ne(T),
    Lt(T),
    Le(T),
    Gt(T),
    Ge(T),
    In(Vec<T>),
}

impl<T: PartialOrd + PartialEq> FilterOp<T> {
    pub fn matches(&self, value: &T) -> bool {
        match self {
            FilterOp::Eq(expected) => value == expected,
            FilterOp::Ne(expected) => value != expected,
            FilterOp::Lt(expected) => value < expected,
            FilterOp::Le(expected) => value <= expected,
            FilterOp::Gt(expected) => value > expected,
            FilterOp::Ge(expected) => value >= expected,
            FilterOp::In(options) => options.contains(value),        }
    }
}

pub fn build_router(state: RequestState, extras: Option<OpenApiRouter<RequestState>>, cors: Option<CorsLayer>) -> Router<()> {
    let mut router: OpenApiRouter<RequestState> = OpenApiRouter::with_openapi(ApiDoc::openapi());
    for info in inventory::iter::<StructInfo> {
        router = router.merge((info.routes_fn)());
    }

    if let Some(extra) = extras {
        router = router.merge(extra);
    }
    let (r, openapi) = router.split_for_parts();

    let merged = r
        .merge(SwaggerUi::new("/swagger-ui").url("/apidoc/openapi.json", openapi))
        .with_state(state);
    if let Some(cors_layer) = cors {
        merged.layer(cors_layer)
    } else {
        merged
    }
}

pub async fn serve(
    state: RequestState,
    socket_addr: SocketAddr,
    extras: Option<OpenApiRouter<RequestState>>,
    cors: Option<CorsLayer>,
    shutdown: watch::Receiver<bool>,
) {
    let router: Router<()> = build_router(state, extras, cors);
    let tcp = TcpListener::bind(socket_addr).await.unwrap();

    // spawn shutdown watcher future
    let mut shutdown = shutdown.clone();
    axum::serve(tcp, router)
        .with_graceful_shutdown(async move {
            if shutdown.changed().await.is_ok() {
                info!("Shutting down server...");
            }
        })
        .await
        .unwrap();
}

pub fn assert_sorted<T, I>(items: &[T], label: &str, mut extract: impl FnMut(&T) -> &I)
where
    I: Key + Borrow<I::SelfType<'static>> + 'static,
{
    for w in items.windows(2) {
        let ia = extract(&w[0]);
        let ib = extract(&w[1]);
        let ord = <I as Key>::compare(
            <I as Value>::as_bytes(ia.borrow()).as_ref(),
            <I as Value>::as_bytes(ib.borrow()).as_ref(),
        );
        assert!(matches!(ord, Ordering::Less | Ordering::Equal), "{} must be sorted by key", label);
    }
}
