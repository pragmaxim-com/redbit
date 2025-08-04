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

pub use axum;
pub use axum::body::Body;
pub use axum::extract;
pub use axum::http::StatusCode;
pub use axum::response::IntoResponse;
pub use axum::response::Response;
pub use axum_streams;
pub use axum_test;
pub use chrono;
pub use futures;
pub use futures::stream::{self, StreamExt};
pub use futures_util::stream::TryStreamExt;
pub use http;
pub use http::HeaderValue;
pub use inventory;
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
pub use redb::Database;
pub use redb::MultimapTableDefinition;
pub use redb::ReadTransaction;
pub use redb::ReadableMultimapTable;
pub use redb::ReadableTable;
pub use redb::TableDefinition;
pub use redb::WriteTransaction;
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
use crate::redb::{Key, TypeName, Value};
use crate::utoipa::OpenApi;
use crate::utoipa_swagger_ui::SwaggerUi;
use axum::body::Bytes;
use axum::extract::Request;
use bincode::Options;
use serde::de::DeserializeOwned;
use std::any::type_name;
use std::cmp::Ordering;
use std::env;
use std::fmt::Debug;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::SocketAddr;
use std::ops::Add;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

pub trait IndexedPointer: Clone {
    type Index: Copy + Ord + Add<Output = Self::Index> + Default;
    fn index(&self) -> Self::Index;
    fn next_index(&self) -> Self;
    fn rollback_or_init(&self, n: u32) -> Self;
}

pub trait RootPointer: IndexedPointer {
    fn is_pointer(&self) -> bool;
}

pub trait ChildPointer: IndexedPointer {
    type Parent: IndexedPointer;
    fn is_pointer(&self) -> bool;
    fn parent(&self) -> &Self::Parent;
    fn from_parent(parent: Self::Parent, index: Self::Index) -> Self;
}

pub trait ForeignKey<CH: ChildPointer> {
    fn fk_range(&self) -> (CH, CH);
}

impl<CH> ForeignKey<CH> for CH::Parent
where
    CH: ChildPointer + Clone,
    CH::Parent: IndexedPointer + Clone,
{
    fn fk_range(&self) -> (CH, CH) {
        (CH::from_parent(self.clone(), CH::Index::default()), CH::from_parent(self.clone().next_index(), CH::Index::default()))
    }
}

pub trait IterableColumn: Sized {
    fn next_value(&self) -> Self;
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
    fn encode(&self) -> String;
}

pub trait ByteVecColumnSerde {
    fn decoded_example() -> Vec<u8>;
    fn encoded_example() -> String;
}

#[derive(Debug)]
pub enum AppError {
    Internal(String),
    NotFound(String),
    BadRequest(String),
    JsonRejection(JsonRejection),
}

#[derive(Debug, thiserror::Error)]
pub enum ParsePointerError {
    #[error("invalid pointer format")]
    Format,
    #[error("invalid integer: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Internal(msg) => write!(f, "Database error: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            AppError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
            AppError::JsonRejection(reject) => write!(f, "Json rejection: {}", reject),
        }
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        Self::JsonRejection(rejection)
    }
}

impl Into<axum::Error> for AppError {
    fn into(self) -> axum::Error {
        axum::Error::new(self.to_string())
    }
}

impl std::error::Error for AppError {}

impl From<redb::Error> for AppError {
    fn from(e: redb::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
impl From<http::Error> for AppError {
    fn from(e: http::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
impl From<redb::DatabaseError> for AppError {
    fn from(e: redb::DatabaseError) -> Self {
        AppError::Internal(e.to_string())
    }
}
impl From<redb::TransactionError> for AppError {
    fn from(e: redb::TransactionError) -> Self {
        AppError::Internal(e.to_string())
    }
}
impl From<redb::StorageError> for AppError {
    fn from(e: redb::StorageError) -> Self {
        AppError::Internal(e.to_string())
    }
}
impl From<redb::TableError> for AppError {
    fn from(e: redb::TableError) -> Self {
        AppError::Internal(e.to_string())
    }
}
impl From<redb::CommitError> for AppError {
    fn from(e: redb::CommitError) -> Self {
        AppError::Internal(e.to_string())
    }
}

#[derive(Debug)]
pub struct Bincode<T>(pub T);

impl<T> Value for Bincode<T>
where
    T: Debug + Serialize + for<'a> Deserialize<'a>,
{
    type SelfType<'a>
        = T
    where
        Self: 'a;

    type AsBytes<'a>
        = Vec<u8>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        bincode::options().with_big_endian().with_fixint_encoding().deserialize(data).expect("Unable to deserialize value")
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        bincode::options().with_big_endian().with_fixint_encoding().serialize(value).expect("Unable to serialize value")
    }

    fn type_name() -> TypeName {
        TypeName::new(&format!("Bincode<{}>", type_name::<T>()))
    }
}

impl<T> Key for Bincode<T>
where
    T: Debug + Serialize + DeserializeOwned + Ord,
{
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        Self::from_bytes(data1).cmp(&Self::from_bytes(data2))
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

        let parsed = serde_json::from_slice::<T>(&body_bytes).map_err(|e| AppError::BadRequest(e.to_string()))?;

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
        let (status, message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::JsonRejection(rejection) => (rejection.status(), rejection.body_text()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, AppJson(ErrorResponse { message, code: status.as_u16() })).into_response()
    }
}

#[derive(Clone)]
pub struct RequestState {
    pub db: Arc<Database>,
}

impl RequestState {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
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

pub fn test_db_path(entity_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("redbit").join("test");
    if !dir.exists() {
        std::fs::create_dir_all(dir.clone()).unwrap();
    }
    dir.join(format!("{}_{}.redb", entity_name, rand::random::<u64>()))
}

pub async fn build_test_server(db: Arc<Database>) -> axum_test::TestServer {
    let router = build_router(RequestState { db }, None, None).await;
    axum_test::TestServer::new(router).unwrap()
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

pub async fn serve(state: RequestState, socket_addr: SocketAddr, extras: Option<OpenApiRouter<RequestState>>, cors: Option<CorsLayer>) -> () {
    let router: Router<()> = build_router(state, extras, cors).await;
    println!("Starting server on {}", socket_addr);
    let tcp = TcpListener::bind(socket_addr).await.unwrap();
    crate::axum::serve(tcp, router).await.unwrap();
}

//TODO duplicated with `macros/src/macro_utils.rs`
pub fn write_to_local_file(lines: Vec<String>, dir_name: &str, file_name: &str) {
    let dir_path = env::current_dir().expect("current dir inaccessible").join("target").join("macros").join(dir_name);
    if let Err(e) = std::fs::create_dir_all(&dir_path) {
        eprintln!("Failed to create directory {:?}: {}", dir_path, e);
        return;
    }
    let full_path = dir_path.join(file_name);

    #[cfg(not(test))]
    {
        if let Err(e) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&full_path)
            .and_then(|mut file| file.write_all(lines.join("\n").as_bytes()))
        {
            eprintln!("Failed to write to {:?}: {}", full_path, e);
        }
    }
}
