pub use macros::Entity;
pub use redb::ReadableMultimapTable;
pub use redb::ReadableTable;

use bincode::Options;
use redb::{Key, TypeName, Value};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::any::type_name;
use std::cmp::Ordering;
use std::fmt::Debug;

pub trait PK<FK>: Sized {
    fn fk_range(&self) -> (FK, FK);
}

#[derive(Debug)]
pub enum DbEngineError {
    DbError(String),
}

impl std::fmt::Display for DbEngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbEngineError::DbError(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl std::error::Error for DbEngineError {}

impl From<redb::Error> for DbEngineError {
    fn from(e: redb::Error) -> Self {
        DbEngineError::DbError(e.to_string())
    }
}
impl From<redb::TransactionError> for DbEngineError {
    fn from(e: redb::TransactionError) -> Self {
        DbEngineError::DbError(e.to_string())
    }
}
impl From<redb::StorageError> for DbEngineError {
    fn from(e: redb::StorageError) -> Self {
        DbEngineError::DbError(e.to_string())
    }
}
impl From<redb::TableError> for DbEngineError {
    fn from(e: redb::TableError) -> Self {
        DbEngineError::DbError(e.to_string())
    }
}
impl From<redb::CommitError> for DbEngineError {
    fn from(e: redb::CommitError) -> Self {
        DbEngineError::DbError(e.to_string())
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
        bincode::options().with_big_endian().with_fixint_encoding().deserialize(data).unwrap()
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        bincode::options().with_big_endian().with_fixint_encoding().serialize(value).unwrap()
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
