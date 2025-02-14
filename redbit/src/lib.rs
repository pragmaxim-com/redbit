pub use macros::Redbit;
pub use redb::ReadableTable;

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