use async_trait::async_trait;
use futures::Stream;
use hex::FromHexError;
use redbit::AppError;
use serde::{Deserialize, Serialize};
use std::{fmt, pin::Pin};
use std::sync::Arc;
use chrono::DateTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChainSyncError {
    pub error: String,
}
impl fmt::Display for ChainSyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl From<redb::Error> for ChainSyncError {
    fn from(err: redb::Error) -> Self {
        ChainSyncError::new(&err.to_string())
    }
}

impl From<redb::TransactionError> for ChainSyncError {
    fn from(err: redb::TransactionError) -> Self {
        ChainSyncError::new(&err.to_string())
    }
}
impl From<redb::CommitError> for ChainSyncError {
    fn from(err: redb::CommitError) -> Self {
        ChainSyncError::new(&err.to_string())
    }
}
impl From<AppError> for ChainSyncError {
    fn from(err: AppError) -> Self {
        ChainSyncError::new(&err.to_string())
    }
}
impl From<FromHexError> for ChainSyncError {
    fn from(err: FromHexError) -> Self {
        ChainSyncError::new(&err.to_string())
    }
}

impl ChainSyncError {
    pub fn new(error: &str) -> Self {
        ChainSyncError { error: error.to_string() }
    }
}

pub trait BlockHeaderLike: Send + Sync + Clone {
    fn height(&self) -> u32;
    fn hash(&self) -> [u8; 32];
    fn prev_hash(&self) -> [u8; 32];
    fn timestamp(&self) -> u32;

    fn timestamp_str(&self) -> String {
        let datetime = DateTime::from_timestamp(self.timestamp() as i64, 0).unwrap();
        datetime.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    fn hash_str(&self) -> String {
        hex::encode(self.hash())
    }
}

pub trait BlockLike: Send + Sync {
    type Header: BlockHeaderLike + 'static;
    fn header(&self) -> &Self::Header;
    fn weight(&self) -> u32;
}

pub trait BlockPersistence<B: BlockLike>: Send + Sync {
    fn init(&self) -> Result<(), ChainSyncError>;
    fn get_last_header(&self) -> Result<Option<B::Header>, ChainSyncError>;
    fn get_header_by_hash(&self, hash: [u8; 32]) -> Result<Vec<B::Header>, ChainSyncError>;
    fn store_blocks(&self, blocks: Vec<B>) -> Result<(), ChainSyncError>;
    fn update_blocks(&self, blocks: Vec<B>) -> Result<(), ChainSyncError>;
}

#[async_trait]
pub trait BlockProvider<FB: Send, TB: BlockLike>: Send + Sync {
    fn block_processor(&self) -> Arc<dyn Fn(&FB) -> Result<TB, ChainSyncError> + Send + Sync>;
    fn get_processed_block(&self, header: TB::Header) -> Result<TB, ChainSyncError>;
    async fn get_chain_tip(&self) -> Result<TB::Header, ChainSyncError>;
    fn stream(&self, chain_tip_header: TB::Header, last_header: Option<TB::Header>) -> Pin<Box<dyn Stream<Item = FB> + Send + 'static>>;
}
