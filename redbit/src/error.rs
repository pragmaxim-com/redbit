use std::sync::PoisonError;
use axum::extract::rejection::JsonRejection;
use crossbeam::channel::{RecvError, SendError};
use http::StatusCode;
use thiserror::Error;
use tokio::task::JoinError;

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
    pub fn status_code(&self) -> StatusCode {
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
