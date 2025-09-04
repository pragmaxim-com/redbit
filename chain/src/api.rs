use std::fmt::Display;
use async_trait::async_trait;
use futures::Stream;
use hex::FromHexError;
use redbit::AppError;
use std::pin::Pin;
use std::sync::Arc;
use redb::StorageError;
use tokio::task::JoinError;
use crate::batcher::SyncMode;

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

    #[error("{0}")]
    Custom(String),
}

impl ChainError {
    pub fn new(msg: impl Into<String>) -> Self {
        ChainError::Custom(msg.into())
    }
}
pub trait BlockHeaderLike: Send + Sync + Clone {
    type Hash : Display;
    type TS : Display;
    fn height(&self) -> u32;
    fn hash(&self) -> Self::Hash;
    fn prev_hash(&self) -> Self::Hash;
    fn timestamp(&self) -> Self::TS;
    fn weight(&self) -> u32;
}

pub trait BlockLike: Send + Sync {
    type Header: BlockHeaderLike + 'static;
    fn header(&self) -> &Self::Header;
}

pub trait SizeLike: Send + Sync {
    fn size(&self) -> usize;
}

#[async_trait]
pub trait BlockChainLike<B: BlockLike>: Send + Sync {
    fn init(&self) -> Result<(), ChainError>;
    fn delete(&self) -> Result<(), ChainError>;
    fn get_last_header(&self) -> Result<Option<B::Header>, ChainError>;
    fn get_header_by_hash(&self, hash: <B::Header as BlockHeaderLike>::Hash) -> Result<Vec<B::Header>, ChainError>;
    fn store_blocks(&self, blocks: Vec<B>) -> Result<(), ChainError>;
    fn update_blocks(&self, blocks: Vec<B>) -> Result<(), ChainError>;
    async fn validate_chain(&self, validation_from_height: u32) -> Result<Vec<B::Header>, ChainError>;
}

#[async_trait]
pub trait BlockProvider<FB: SizeLike, TB: BlockLike>: Send + Sync {
    fn block_processor(&self) -> Arc<dyn Fn(&FB) -> Result<TB, ChainError> + Send + Sync>;
    fn get_processed_block(&self, hash: <TB::Header as BlockHeaderLike>::Hash) -> Result<Option<TB>, ChainError>;
    async fn get_chain_tip(&self) -> Result<TB::Header, ChainError>;
    fn stream(&self, remote_chain_tip_header: TB::Header, last_persisted_header: Option<TB::Header>, mode: SyncMode) -> Pin<Box<dyn Stream<Item = FB> + Send + 'static>>;
}
