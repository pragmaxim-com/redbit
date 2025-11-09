use async_trait::async_trait;
use redb::Durability;
use redbit::storage::table_writer_api::TaskResult;
use redbit::WriteTxContext;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use crate::err::ChainError;

pub type Height = u32;

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
pub trait BlockChainLike<B: BlockLike, CTX: WriteTxContext>: Send + Sync {
    fn new_indexing_ctx(&self) -> Result<CTX, ChainError>;
    fn init(&self) -> Result<(), ChainError>;
    fn delete(&self) -> Result<(), ChainError>;
    fn get_last_header(&self) -> Result<Option<B::Header>, ChainError>;
    fn get_header_by_hash(&self, hash: <B::Header as BlockHeaderLike>::Hash) -> Result<Vec<B::Header>, ChainError>;
    fn store_blocks(&self, indexing_context: &CTX, blocks: Vec<B>, durability: Durability) -> Result<HashMap<String, TaskResult>, ChainError>;
    fn update_blocks(&self, indexing_context: &CTX, blocks: Vec<B>) -> Result<HashMap<String, TaskResult>, ChainError>;
    async fn validate_chain(&self, validation_from_height: u32) -> Result<Vec<B::Header>, ChainError>;
}

#[async_trait]
pub trait BlockProvider<FB: SizeLike, TB: BlockLike>: Send + Sync {
    fn block_processor(&self) -> Arc<dyn Fn(&FB) -> Result<TB, ChainError> + Send + Sync>;
    fn get_processed_block(&self, hash: <TB::Header as BlockHeaderLike>::Hash) -> Result<Option<TB>, ChainError>;
    async fn get_chain_tip(&self) -> Result<TB::Header, ChainError>;
    fn block_stream(&self, remote_chain_tip_header: TB::Header, last_persisted_header: Option<TB::Header>, shutdown: watch::Receiver<bool>, batch: bool) -> (Receiver<Vec<FB>>, JoinHandle<()>);
}
