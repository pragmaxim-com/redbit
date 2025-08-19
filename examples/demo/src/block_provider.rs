use crate::model_v1::stream::Stream;
use crate::model_v1::{Block, Header, Height, StreamExt, Weight};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use redbit::info;
use chain::api::{BlockProvider, ChainError};
use chain::batcher::SyncMode;

pub struct DemoBlockProvider {
    pub chain: Arc<RwLock<BTreeMap<Height, Block>>>,
}

impl DemoBlockProvider {
    pub fn new(chain_tip_height: usize) -> Result<Arc<Self>> {
        assert!(chain_tip_height < 100_000, "Chain height must be less than 100_000");
        let mut chain = BTreeMap::new();
        let mut blocks_iter = Block::sample_many(chain_tip_height + 1).into_iter();
        let genesis = blocks_iter.next().expect("at least one block");
        let mut prev_hash = genesis.header.hash.clone();
        for mut block in blocks_iter {
            block.header.prev_hash = prev_hash.clone();
            block.header.weight = Weight(block
                .transactions
                .iter()
                .flat_map(|t| &t.utxos)
                .map(|u| u.assets.len() as u32)
                .sum());
            prev_hash = block.header.hash.clone();
            chain.insert(block.header.height, block);
        }
        info!("Demo chain initialized with {} blocks", chain.len());
        Ok(Arc::new(DemoBlockProvider { chain: Arc::new(RwLock::new(chain)) }))
    }
}

#[async_trait]
impl BlockProvider<Block, Block> for DemoBlockProvider {
    fn block_processor(&self) -> Arc<dyn Fn(&Block) -> std::result::Result<Block, ChainError> + Send + Sync> {
        Arc::new(|block| Ok(block.clone()))
    }

    fn get_processed_block(&self, header: Header) -> Result<Block, ChainError> {
        let chain = self.chain.read().unwrap();
        let result = chain.get(&header.height).ok_or_else(|| ChainError::new("Block not found"))?;
        Ok(result.clone())
    }

    async fn get_chain_tip(&self) -> Result<Header, ChainError> {
        let chain = self.chain.read().unwrap();
        let (_, tip_block) = chain.last_key_value().ok_or_else(|| ChainError::new("No blocks in chain"))?;
        Ok(tip_block.header.clone())
    }

    fn stream(
        &self,
        remote_chain_tip_header: Header,
        last_persisted_header: Option<Header>,
        _mode: SyncMode
    ) -> Pin<Box<dyn Stream<Item = Block> + Send + 'static>> {
        let height_to_index_from = last_persisted_header.map_or(1, |h| h.height.0 + 1);
        let heights = height_to_index_from..=remote_chain_tip_header.height.0;
        let chain = self.chain.clone();
        tokio_stream::iter(heights)
            .map(move |height| {
                let chain = Arc::clone(&chain);
                chain.read().unwrap().get(&Height(height)).expect("Failed to fetch block by height").clone()
            })
            .boxed()
    }
}