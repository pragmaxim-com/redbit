use crate::model_v1::{Block, Height, Header};
use async_trait::async_trait;
use chain::api::{BlockChainLike, BlockLike};
use chain::err::ChainError;
use redbit::redb;
use redbit::Durability;
use redbit::manual_entity::{ManualEntityRuntime, RuntimeWritersWithChildren};
use redbit::storage::context::{TxContext, ReadTxContext, WriteTxContext};
use redbit::storage::init::Storage;
use redbit::storage::table_writer_api::TaskResult;
use redbit::{info, AppError};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ManualChain {
    storage: Arc<Storage>,
    block_rt: Arc<ManualEntityRuntime<Block, Height>>,
    defs: Arc<ManualDefs>,
}

#[derive(Clone)]
pub struct ManualDefs {
    pub block_rt: Arc<ManualEntityRuntime<Block, Height>>,
}

impl TxContext for ManualDefs {
    type ReadCtx = ManualReadCtx;
    type WriteCtx = ManualCtx;
    fn definition() -> redb::Result<Self, AppError> { Err(AppError::Custom("ManualDefs requires runtime".into())) }
}

pub struct ManualReadCtx {
    _storage: Arc<Storage>,
}

impl ReadTxContext for ManualReadCtx {
    type Defs = ManualDefs;
    fn begin_read_ctx(_defs: &Self::Defs, storage: &Arc<Storage>) -> redb::Result<Self, AppError> where Self: Sized {
        Ok(ManualReadCtx { _storage: Arc::clone(storage) })
    }
}

pub struct ManualCtx {
    pub(crate) storage: Arc<Storage>,
    pub(crate) block_writers: RuntimeWritersWithChildren<Block, Height>,
}

impl WriteTxContext for ManualCtx {
    type Defs = ManualDefs;
    type WriterRefs<'a> = Vec<&'a dyn redbit::storage::table_writer_api::WriteComponentRef> where Self: 'a;
    fn new_write_ctx(_defs: &Self::Defs, storage: &Arc<Storage>) -> redb::Result<Self, AppError> where Self: Sized {
        let block_writers = Block::manual_writers_auto(storage)?;
        Ok(ManualCtx { storage: Arc::clone(storage), block_writers })
    }
    fn stop_writing_async(self) -> redb::Result<Vec<redbit::storage::table_writer_api::StopFuture>, AppError> {
        self.block_writers.stop_async()
    }
    fn writer_refs(&self) -> Self::WriterRefs<'_> { self.block_writers.writer_refs() }

    fn begin_writing_async(&self, d: Durability) -> redb::Result<Vec<redbit::storage::table_writer_api::StartFuture>, AppError> {
        self.block_writers.begin_all(d)
    }

    fn commit_ctx_async(&self) -> Result<Vec<redbit::storage::table_writer_api::FlushFuture>, AppError> {
        self.block_writers.commit_all()
    }
}

#[async_trait]
impl BlockChainLike<Block, ManualCtx> for ManualChain {
    fn new_indexing_ctx(&self) -> Result<ManualCtx, ChainError> {
        ManualCtx::new_write_ctx(&self.defs, &self.storage).map_err(|e| ChainError::new(format!("ctx: {e}")))
    }

    fn init(&self) -> Result<(), ChainError> {
        // ensure tables by opening writer tree once
        let writers = Block::manual_writers_auto(&self.storage).map_err(|e| ChainError::new(format!("init writers: {e}")))?;
        let stops = writers.stop_async().map_err(|e| ChainError::new(format!("init stop: {e}")))?;
        for s in stops {
            s.wait().map_err(|e| ChainError::new(format!("init stop wait: {e}")))?;
        }
        Ok(())
    }

    fn delete(&self) -> Result<(), ChainError> {
        // manual delete not implemented
        Ok(())
    }

    fn get_last_header(&self) -> Result<Option<<Block as BlockLike>::Header>, ChainError> {
        let ctx = Header::begin_read_ctx(&self.storage).map_err(|e| ChainError::new(format!("header ctx: {e}")))?;
        Header::last(&ctx).map_err(|e| ChainError::new(format!("header last: {e}")))
    }

    fn get_header_by_hash(&self, _hash: crate::model_v1::BlockHash) -> Result<Vec<<Block as BlockLike>::Header>, ChainError> {
        let ctx = Header::begin_read_ctx(&self.storage).map_err(|e| ChainError::new(format!("header ctx: {e}")))?;
        Header::get_by_hash(&ctx, &_hash).map_err(|e| ChainError::new(format!("header get_by_hash: {e}")))
    }

    fn store_blocks(&self, indexing_context: &ManualCtx, blocks: Vec<Block>, durability: Durability) -> Result<HashMap<String, TaskResult>, ChainError> {
        if blocks.is_empty() { return Ok(HashMap::new()); }
        indexing_context.begin_writing(durability).map_err(|e| ChainError::new(format!("begin_writing: {e}")))?;
        let tasks = indexing_context.two_phase_commit_with(|ctx| {
            self.block_rt.store_batch_with_writer_tree(&ctx.storage, &ctx.block_writers, &blocks)
        }).map_err(|e| ChainError::new(format!("commit: {e}")))?;
        Ok(tasks)
    }

    fn update_blocks(&self, ctx: &ManualCtx, blocks: Vec<Block>) -> Result<HashMap<String, TaskResult>, ChainError> {
        self.store_blocks(ctx, blocks, Durability::None)
    }

    async fn validate_chain(&self, _validation_from_height: u32) -> Result<Vec<<Block as BlockLike>::Header>, ChainError> {
        Ok(Vec::new())
    }
}

impl ManualChain {
    pub fn new(storage: Arc<Storage>) -> Arc<Self> {
        info!("Building manual runtimes");
        let block_rt = Block::manual_runtime_auto().expect("block runtime");
        let defs = Arc::new(ManualDefs { block_rt: Arc::clone(&block_rt) });
        Arc::new(ManualChain { storage, block_rt, defs })
    }
}

pub fn build_block_chain_auto(storage: Arc<Storage>) -> Arc<dyn BlockChainLike<Block, ManualCtx>> {
    ManualChain::new(storage) as Arc<dyn BlockChainLike<Block, ManualCtx>>
}
