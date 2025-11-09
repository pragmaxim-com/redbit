use config::ConfigError;
use hex::FromHexError;
use redb::StorageError;
use tokio::task::JoinError;
use redbit::AppError;

#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    #[error("Join error: {0}")]
    JoinError(#[from] JoinError),

    #[error("Shutdown: {0}")]
    Shutdown(String),

    #[error("Database error: {0}")]
    Redb(#[from] redb::Error),

    #[error("Transaction error: {0}")]
    RedbTransaction(#[from] redb::TransactionError),

    #[error("Transaction error: {0}")]
    RedbTable(#[from] redb::TableError),

    #[error("Commit error: {0}")]
    RedbCommit(#[from] redb::CommitError),

    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),

    #[error("Application error: {0}")]
    App(#[from] AppError),

    #[error("Invalid hex: {0}")]
    Hex(#[from] FromHexError),

    #[error("Config error: {0}")]
    ConfigError(#[from] ConfigError),

    #[error("{0}")]
    Custom(String),
}

impl ChainError {
    pub fn new(msg: impl Into<String>) -> Self {
        ChainError::Custom(msg.into())
    }
}
