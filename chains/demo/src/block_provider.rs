use crate::model_v1::{Block, BlockHash, Header, Height, Weight};
use anyhow::Result;
use async_trait::async_trait;
use chain::api::{BlockProvider, ChainError, SizeLike};
use chain::block_stream::{BlockStream, RestBlockStream, RestClient};
use chain::settings::{AppConfig, Parallelism};
use redbit::info;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::Receiver;
use tokio::sync::watch;
use tokio::task::JoinHandle;

impl SizeLike for Block {
    fn size(&self) -> usize {
        512 // Dummy fixed size for demo purposes
    }
}

pub struct DemoClient {
    pub hash_by_height: Arc<RwLock<BTreeMap<Height, BlockHash>>>,
    pub block_by_hash: Arc<RwLock<HashMap<BlockHash, Block>>>,
}

#[async_trait::async_trait]
impl RestClient<Block> for DemoClient {
    async fn get_block_by_height(&self, height: chain::api::Height) -> std::result::Result<Block, ChainError> {
        let hash_by_height = self.hash_by_height.read().unwrap();
        let block_by_hash = self.block_by_hash.read().unwrap();
        let hash = hash_by_height.get(&Height(height)).ok_or_else(|| ChainError::new("Block not found by height"))?;
        let block = block_by_hash.get(hash).ok_or_else(|| ChainError::new("Block not found by hash"))?;
        Ok(block.clone())
    }
}

pub struct DemoBlockProvider {
    pub client: Arc<DemoClient>,
    pub block_stream: Arc<dyn BlockStream<Block, Block>>,
}

impl DemoBlockProvider {
    pub fn for_height(chain_tip_height: u32, max_entity_buffer_kb_size: usize) -> Result<Arc<dyn BlockProvider<Block, Block>>, ChainError> {
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
        let client = Arc::new(DemoClient {
            hash_by_height: Arc::new(RwLock::new(hash_by_height.clone())),
            block_by_hash: Arc::new(RwLock::new(block_by_hash.clone())),
        });
        let block_stream = Arc::new(RestBlockStream::new(Arc::clone(&client), Parallelism(2), max_entity_buffer_kb_size));
        Ok(Arc::new(DemoBlockProvider { client, block_stream } ))
    }

    pub fn new(config: AppConfig) -> Result<Arc<dyn BlockProvider<Block, Block>>, ChainError> {
        Self::for_height(1005, config.indexer.max_entity_buffer_kb_size)
    }

    fn calculate_weight(b: &Block) -> Weight {
        Weight(6 + b
            .transactions
            .iter()
            .map(|t| t.inputs.len() + t.utxos.len() + t.utxos.iter().map(|u| u.assets.len()).sum::<usize>() + 1)
            .sum::<usize>() as u32
        )
    }
}

#[async_trait]
impl BlockProvider<Block, Block> for DemoBlockProvider {
    fn block_processor(&self) -> Arc<dyn Fn(&Block) -> std::result::Result<Block, ChainError> + Send + Sync> {
        Arc::new(|block| Ok(block.clone()))
    }

    fn get_processed_block(&self, hash: BlockHash) -> Result<Option<Block>, ChainError> {
        let block_by_hash = self.client.block_by_hash.read().unwrap();
        Ok(block_by_hash.get(&hash).cloned())
    }

    async fn get_chain_tip(&self) -> Result<Header, ChainError> {
        let hash_by_height = self.client.hash_by_height.read().unwrap();
        let block_by_hash = self.client.block_by_hash.read().unwrap();
        let (_, tip_hash) = hash_by_height.last_key_value().ok_or_else(|| ChainError::new("No blocks in chain"))?;
        Ok(block_by_hash.get(tip_hash).unwrap().header.clone())
    }

    fn block_stream(
        &self,
        remote_chain_tip_header: Header,
        last_persisted_header: Option<Header>,
        shutdown: watch::Receiver<bool>,
        batch: bool
    ) -> (Receiver<Vec<Block>>, JoinHandle<()>) {
        self.block_stream.stream(remote_chain_tip_header, last_persisted_header, shutdown, batch)
    }
}