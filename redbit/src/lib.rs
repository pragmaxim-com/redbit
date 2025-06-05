//! redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from
//! [Redb](https://github.com/cberner/redb) using secondary indexes and dictionaries.
//!
//! It leverages the `redb` crate for storage, with custom implementations for serializing and deserializing data using `bincode`.
//! The library provides methods for storing, retrieving, and deleting entities based on primary keys (PKs) and secondary indexes,
//! supporting one-to-one and one-to-many relationships.
//!
pub use macros::Entity;
pub use macros::PK;
pub use redb::ReadableMultimapTable;
pub use redb::ReadableTable;
pub use inventory;
pub use axum;

use bincode::Options;
use redb::{Key, TypeName, Value};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::any::type_name;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::ops::Add;
use axum::extract::FromRequest;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Router;
use tokio::net::TcpListener;
use std::sync::Arc;
use redb::Database;

pub trait IndexedPointer: Clone {
    type Index: Copy + Ord + Add<Output = Self::Index> + Default;
    fn index(&self) -> Self::Index;
    fn next(&self) -> Self;
}

pub trait RootPointer: IndexedPointer {}

pub trait ChildPointer: IndexedPointer {
    type Parent: IndexedPointer;
    fn parent(&self) -> &Self::Parent;
    fn from_parent(parent: Self::Parent) -> Self;
}

pub trait ForeignKey<T: ChildPointer> {
    fn fk_range(&self) -> (T, T);
}

impl<T> ForeignKey<T> for T::Parent
where
    T: ChildPointer + Clone,
    T::Parent: IndexedPointer + Clone,
{
    fn fk_range(&self) -> (T, T) {
        (T::from_parent(self.clone()), T::from_parent(self.clone().next()))
    }
}

#[derive(Debug)]
pub enum AppError {
    Internal(String),
    NotFound(String),
    JsonRejection(JsonRejection),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Internal(msg) => write!(f, "Database error: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            AppError::JsonRejection(reject) => write!(f, "Json rejection: {}", reject),
        }
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        Self::JsonRejection(rejection)
    }
}

impl std::error::Error for AppError {}

impl From<redb::Error> for AppError {
    fn from(e: redb::Error) -> Self {
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

#[derive(Deserialize)]
pub struct RangeParams<F, T> {
    pub from: F,
    pub to: T,
}

#[derive(Deserialize)]
pub struct ByParams<B> {
    pub value: B,
}

// Create our own JSON extractor by wrapping `axum::Json`. This makes it easy to override the
// rejection and provide our own which formats errors to match our application.
//
// `axum::Json` responds with plain text if the input is invalid.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct AppJson<T>(pub T);

impl<T> IntoResponse for AppJson<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        #[derive(Serialize)]
        struct ErrorResponse {
            message: String,
        }

        let (status, message) = match self {
            AppError::NotFound(msg) =>
                (StatusCode::NOT_FOUND, msg),
            AppError::JsonRejection(rejection) => 
                (rejection.status(), rejection.body_text()),
            AppError::Internal(msg) =>
                (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, AppJson(ErrorResponse { message })).into_response()
    }
}

#[derive(Clone)]
pub struct RequestState {
    pub db: Arc<Database>
}

impl RequestState {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }}

pub struct EntityInfo {
    pub name: &'static str,
    pub routes_fn: fn() -> axum::Router<RequestState>,
}

inventory::collect!(EntityInfo);

pub async fn build_router(state: RequestState) -> Router<()> {
    let mut router = Router::new();
    for reg in inventory::iter::<EntityInfo> {
        router = router.merge((reg.routes_fn)());
    }
    router.with_state(state)
}

pub async fn serve(state: RequestState, socket_addr: SocketAddr) -> () {
    let router: Router<()> = build_router(state).await;
    println!("Starting server on {}", socket_addr);
    let tcp = TcpListener::bind(socket_addr).await.unwrap();
    axum::serve(tcp, router).await.unwrap();
}