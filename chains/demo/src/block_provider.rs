use crate::model_v1::stream::Stream;
use crate::model_v1::{Block, BlockHash, Header, Height, StreamExt, Weight};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use redbit::info;
use chain::api::{BlockProvider, ChainError, SizeLike};
use crate::model_v1::redb::Durability;

impl SizeLike for Block {
    fn size(&self) -> usize {
        512 // Dummy fixed size for demo purposes
    }
}

pub struct DemoBlockProvider {
    pub hash_by_height: Arc<RwLock<BTreeMap<Height, BlockHash>>>,
    pub block_by_hash: Arc<RwLock<HashMap<BlockHash, Block>>>,
}

impl DemoBlockProvider {
    pub fn new(chain_tip_height: u32) -> Result<Arc<Self>> {
        assert!(chain_tip_height < 100_000, "Chain height must be less than 100_000");
        let mut hash_by_height = BTreeMap::new();
        let mut block_by_hash = HashMap::new();
        let mut blocks_iter = Block::sample_many(chain_tip_height as usize + 1).into_iter();
        let genesis = blocks_iter.next().expect("at least one block");
        let mut prev_hash = genesis.header.hash;
        for mut block in blocks_iter {
            block.header.prev_hash = prev_hash;
            block.header.weight = Self::calculate_weight(&block);
            prev_hash = block.header.hash;
            hash_by_height.insert(block.header.height, block.header.hash);
            block_by_hash.insert(block.header.hash, block);
        }
        info!("Demo chain initialized with {} blocks", hash_by_height.len());
        Ok(Arc::new(DemoBlockProvider { hash_by_height: Arc::new(RwLock::new(hash_by_height)), block_by_hash: Arc::new(RwLock::new(block_by_hash)) }))
    }

    fn calculate_weight(b: &Block) -> Weight {
        Weight(b
            .transactions
            .iter()
            .flat_map(|t| &t.utxos)
            .map(|u| u.assets.len() as u32)
            .sum())
    }
}

#[async_trait]
impl BlockProvider<Block, Block> for DemoBlockProvider {
    fn block_processor(&self) -> Arc<dyn Fn(&Block) -> std::result::Result<Block, ChainError> + Send + Sync> {
        Arc::new(|block| Ok(block.clone()))
    }

    fn get_processed_block(&self, hash: BlockHash) -> Result<Option<Block>, ChainError> {
        let block_by_hash = self.block_by_hash.read().unwrap();
        Ok(block_by_hash.get(&hash).cloned())
    }

    async fn get_chain_tip(&self) -> Result<Header, ChainError> {
        let hash_by_height = self.hash_by_height.read().unwrap();
        let block_by_hash = self.block_by_hash.read().unwrap();
        let (_, tip_hash) = hash_by_height.last_key_value().ok_or_else(|| ChainError::new("No blocks in chain"))?;
        Ok(block_by_hash.get(tip_hash).unwrap().header.clone())
    }

    fn stream(
        &self,
        remote_chain_tip_header: Header,
        last_persisted_header: Option<Header>,
        _durability: Durability
    ) -> Pin<Box<dyn Stream<Item = Block> + Send + 'static>> {
        let height_to_index_from = last_persisted_header.map_or(1, |h| h.height.0 + 1);
        let heights = height_to_index_from..=remote_chain_tip_header.height.0;
        let hash_by_height = self.hash_by_height.clone();
        let block_by_hash = self.block_by_hash.clone();
        tokio_stream::iter(heights)
            .map(move |height| {
                let hash_by_height = Arc::clone(&hash_by_height);
                let block_by_hash = Arc::clone(&block_by_hash);
                let hash_by_height = hash_by_height.read().unwrap();
                let block_by_hash = block_by_hash.read().unwrap();
                let hash = hash_by_height.get(&Height(height)).expect("Failed to fetch block hash by height");
                block_by_hash.get(hash).expect("Failed to fetch block by hash").clone()
            })
            .boxed()
    }
}