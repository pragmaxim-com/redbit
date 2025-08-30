//! redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from
//! [Redb](https://github.com/cberner/redb) using secondary indexes and dictionaries.
//!
//! It leverages the `redb` crate for storage, with custom implementations for serializing and deserializing data using `bincode`.
//! The library provides methods for storing, retrieving, and deleting entities based on primary keys (PKs) and secondary indexes,
//! supporting one-to-one and one-to-many relationships.
//!

pub mod query;
pub mod utf8_serde_enc;
pub mod hex_serde_enc;
pub mod base64_serde_enc;
pub mod cache;
pub mod storage;
pub mod retry;
pub mod logger;

pub use axum;
pub use axum::body::Body;
pub use axum::extract;
pub use axum::http::StatusCode;
pub use axum::response::IntoResponse;
pub use axum::response::Response;
pub use axum_streams;
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
pub use rand;
pub use redb;
pub use redb::MultimapTableDefinition;
pub use redb::MultimapTable;
pub use redb::ReadableMultimapTable;
pub use redb::ReadOnlyMultimapTable;
pub use redb::ReadOnlyTable;
pub use redb::ReadTransaction;
pub use redb::TransactionError;
pub use redb::TableError;
pub use redb::ReadableTableMetadata;
pub use redb::ReadableTable;
pub use redb::ReadableDatabase;
pub use redb::TableDefinition;
pub use redb::Table;
pub use redb::Database;
pub use redb::WriteTransaction;
pub use redb::{Key, TypeName, Value};
pub use serde;
pub use serde::Deserialize;
pub use serde::Deserializer;
pub use serde::Serialize;
pub use serde::Serializer;
pub use serde_json;
pub use serde_urlencoded;
pub use serde_with;
pub use std::collections::VecDeque;
pub use std::pin::Pin;
pub use std::sync::Arc;
pub use std::time::Duration;
pub use urlencoding;
pub use utoipa;
pub use utoipa::openapi;
pub use utoipa::IntoParams;
pub use utoipa::PartialSchema;
pub use utoipa::ToSchema;
pub use utoipa_axum;
pub use utoipa_axum::router::OpenApiRouter;
pub use utoipa_swagger_ui;
pub use storage::Storage;
pub use storage::ReadTxContext;
pub use storage::WriteTxContext;
pub use bincode::{Encode, Decode, decode_from_slice, encode_to_vec};
pub use std::any::type_name;
pub use std::cmp::Ordering;
pub use std::fmt::Debug;

use crate::axum::extract::rejection::JsonRejection;
use crate::axum::extract::FromRequest;
use crate::axum::Router;
use crate::utoipa::OpenApi;
use crate::utoipa_swagger_ui::SwaggerUi;
use axum::body::Bytes;
use axum::extract::Request;
use serde::de::DeserializeOwned;
use std::net::SocketAddr;
use std::ops::Add;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::watch;
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

pub trait IterableColumn: Sized + Clone {
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
}
macro_rules! impl_iterable_column_for_primitive {
    ($($t:ty),*) => {
        $(
            impl IterableColumn for $t {
                fn next_value(&self) -> Self {
                    self.wrapping_add(1)
                }
            }
        )*
    };
}

impl_iterable_column_for_primitive!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

pub trait UrlEncoded {
    fn url_encode(&self) -> String;
}

pub trait BinaryCodec {
    fn from_bytes(bytes: &[u8]) -> Self;
    fn as_bytes(&self) -> Vec<u8>;
    fn size() -> usize;
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TableInfo {
    pub table_name: String,
    pub table_type: String,
    pub tree_height: u32,
    pub leaf_pages: u64,
    pub branch_pages: u64,
    pub stored_leaf_bytes: u64,
    pub metadata_bytes: u64,
    pub fragmented_bytes: u64,
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

    #[error("Not Found: {0}")]
    NotFound(String),

    #[error("Bad Request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
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

impl RequestState {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }
}

pub struct StructInfo {
    pub name: &'static str,
    pub root: bool,
    pub routes_fn: fn() -> OpenApiRouter<RequestState>,
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

pub fn create_random_storage(entity_name: &str) -> Arc<Storage> {
    let dir = std::env::temp_dir().join("redbit").join("test");
    if !dir.exists() {
        std::fs::create_dir_all(dir.clone()).unwrap();
    }
    let db = Database::create(dir.join(format!("{}_{}.redb", entity_name, rand::random::<u64>()))).expect("Failed to create test database");
    Arc::new(Storage::new(Arc::new(db)))
}

pub async fn build_router(state: RequestState, extras: Option<OpenApiRouter<RequestState>>, cors: Option<CorsLayer>) -> Router<()> {
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
    let router: Router<()> = build_router(state, extras, cors).await;
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
