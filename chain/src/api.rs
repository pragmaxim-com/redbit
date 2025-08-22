use async_trait::async_trait;
use chrono::DateTime;
use futures::Stream;
use hex::FromHexError;
use redbit::AppError;
use std::pin::Pin;
use std::sync::Arc;
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

    #[error("Commit error: {0}")]
    RedbCommit(#[from] redb::CommitError),

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
    fn height(&self) -> u32;
    fn hash(&self) -> [u8; 32];
    fn prev_hash(&self) -> [u8; 32];
    fn timestamp(&self) -> u32;
    fn weight(&self) -> u32;

    fn timestamp_str(&self) -> String {
        let datetime = DateTime::from_timestamp(self.timestamp() as i64, 0).unwrap();
        datetime.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    fn hash_str(&self) -> String {
        hex::encode(self.hash())
    }

    fn prev_hash_str(&self) -> String {
        hex::encode(self.prev_hash())
    }
}

pub trait BlockLike: Send + Sync {
    type Header: BlockHeaderLike + 'static;
    fn header(&self) -> &Self::Header;
}

#[async_trait]
pub trait BlockChainLike<B: BlockLike>: Send + Sync {
    fn init(&self) -> Result<(), ChainError>;
    fn delete(&self) -> Result<(), ChainError>;
    fn get_last_header(&self) -> Result<Option<B::Header>, ChainError>;
    fn get_header_by_hash(&self, hash: [u8; 32]) -> Result<Vec<B::Header>, ChainError>;
    fn store_blocks(&self, blocks: Vec<B>) -> Result<(), ChainError>;
    fn update_blocks(&self, blocks: Vec<B>) -> Result<(), ChainError>;
    fn populate_inputs(&self, blocks: &mut Vec<B>) -> Result<(), ChainError>;
    async fn validate_chain(&self) -> Result<Vec<B::Header>, ChainError>;
}

#[async_trait]
pub trait BlockProvider<FB: Send, TB: BlockLike>: Send + Sync {
    fn block_processor(&self) -> Arc<dyn Fn(&FB) -> Result<TB, ChainError> + Send + Sync>;
    fn get_processed_block(&self, hash: [u8; 32]) -> Result<Option<TB>, ChainError>;
    async fn get_chain_tip(&self) -> Result<TB::Header, ChainError>;
    fn stream(&self, remote_chain_tip_header: TB::Header, last_persisted_header: Option<TB::Header>, mode: SyncMode) -> Pin<Box<dyn Stream<Item = FB> + Send + 'static>>;
}
