use crate::config::ErgoConfig;
use crate::ergo_client::ErgoClient;
use crate::model_v1;
use crate::model_v1::{Address, Asset, AssetAction, AssetName, AssetType, Block, BlockHash, BlockHeader, BlockPointer, BlockTimestamp, ExplorerError, Height, Transaction, TransactionPointer, TxHash, Utxo, UtxoPointer};
use async_trait::async_trait;
use ergo_lib::chain::transaction::TxIoVec;
use ergo_lib::ergotree_ir::chain::address::AddressEncoder;
use ergo_lib::{chain::block::FullBlock, wallet::signing::ErgoTransaction};
use ergo_lib::{
    ergotree_ir::{
        chain::{address, ergo_box::ErgoBox, token::TokenId},
        serialization::SigmaSerializable,
    },
    wallet::box_selector::ErgoBoxAssets,
};
use futures::stream::StreamExt;
use futures::Stream;
use redbit::*;
use reqwest::Url;
use std::{pin::Pin, str::FromStr, sync::Arc};
use syncer::api::{BlockProvider, ChainSyncError};
use syncer::monitor::BoxWeight;

pub struct ErgoBlockProvider {
    pub client: Arc<ErgoClient>,
    pub fetching_par: usize,
}

impl ErgoBlockProvider {
    pub fn new() -> Result<Arc<Self>, ExplorerError> {
        let ergo_config = ErgoConfig::new("config/ergo").expect("Failed to load Ergo configuration");
        let ergo_client = ErgoClient::new(Url::from_str(&ergo_config.api_host).unwrap(), ergo_config.api_key.clone())?;

        Ok(Arc::new(ErgoBlockProvider {
            client: Arc::new(ergo_client),
            fetching_par: ergo_config.fetching_parallelism.clone().into(),
        }))
    }

    fn process_block_pure(b: &FullBlock) -> Result<Block, ChainSyncError> {
        let mut block_weight: usize = 0;
        let mut result_txs = Vec::with_capacity(b.block_transactions.transactions.len());

        let block_hash: [u8; 32] = b.header.id.0.into();
        let prev_block_hash: [u8; 32] = b.header.parent_id.0.into();

        let height = Height(b.header.height);
        let header = BlockHeader {
            height,
            timestamp: BlockTimestamp((b.header.timestamp / 1000) as u32),
            hash: BlockHash(block_hash),
            prev_hash: BlockHash(prev_block_hash),
        };

        for (tx_index, tx) in b.block_transactions.transactions.iter().enumerate() {
            let tx_hash: [u8; 32] = tx.id().0.0;
            let tx_id = BlockPointer::from_parent(height, tx_index as u16);
            let (box_weight, outputs) = Self::process_outputs(&tx.outputs(), &tx_id);
            let mut inputs = Vec::with_capacity(tx.inputs.len());
            for input in &tx.inputs {
                inputs.push(model_v1::BoxId(input.box_id.as_ref().into()));
            }
            block_weight += box_weight;
            block_weight += tx.inputs.len();
            result_txs.push(Transaction { id: tx_id, hash: TxHash(tx_hash), utxos: outputs, inputs: Vec::new(), transient_inputs: inputs })
        }

        Ok(Block { height, header, transactions: result_txs, weight: block_weight as u32 })
    }

    fn process_outputs(outs: &TxIoVec<ErgoBox>, tx_pointer: &BlockPointer) -> (BoxWeight, Vec<Utxo>) {
        let mut result_outs = Vec::with_capacity(outs.len());
        let mut asset_count = 0;
        for (out_index, out) in outs.iter().enumerate() {
            let box_id = out.box_id();
            let box_id_slice: &[u8] = box_id.as_ref();
            let ergo_tree_opt = out.ergo_tree.sigma_serialize_bytes().ok();
            let ergo_tree_template_opt = out.ergo_tree.template_bytes().ok();
            let address_opt =
                address::Address::recreate_from_ergo_tree(&out.ergo_tree)
                    .map(|a| AddressEncoder::encode_address_as_bytes(crate::codec::MAINNET, &a))
                    .ok();

            let address = Address(address_opt.unwrap_or_default());
            let tree = model_v1::Tree(ergo_tree_opt.unwrap_or_default());
            let tree_template = model_v1::TreeTemplate(ergo_tree_template_opt.unwrap_or_default());

            let amount = *out.value.as_u64();
            let utxo_pointer = TransactionPointer::from_parent(*tx_pointer, out_index as u16);

            let assets: Vec<Asset> = if let Some(assets) = out.tokens() {
                let mut result = Vec::with_capacity(assets.len());
                for (index, asset) in assets.enumerated() {
                    let asset_id: Vec<u8> = asset.token_id.into();
                    let amount = asset.amount;
                    let amount_u64: u64 = amount.into();
                    let new_token_id: TokenId = outs.first().box_id().into();
                    let is_mint = new_token_id == asset.token_id;

                    let action = match is_mint {
                        true => AssetType::Mint, // TODO!! for Minting it might not be enough to check first boxId
                        _ => AssetType::Transfer,
                    };
                    let asset_pointer = UtxoPointer::from_parent(utxo_pointer, index as u8);
                    result.push(Asset { id: asset_pointer, name: AssetName(asset_id), amount: amount_u64, asset_action: AssetAction(action.into()) });
                }
                result
            } else {
                Vec::new()
            };

            asset_count += assets.len();
            result_outs.push(Utxo {
                id: utxo_pointer,
                assets,
                address,
                amount,
                box_id: model_v1::BoxId(box_id_slice.into()),
                tree,
                tree_template,
            })
        }
        (asset_count + result_outs.len(), result_outs)
    }
}

#[async_trait]
impl BlockProvider<FullBlock, Block> for ErgoBlockProvider {

    fn block_processor(&self) -> Arc<dyn Fn(&FullBlock) -> Result<Block, ChainSyncError> + Send + Sync> {
        Arc::new(|raw| ErgoBlockProvider::process_block_pure(raw))
    }

    fn get_processed_block(&self, header: BlockHeader) -> Result<Block, ChainSyncError> {
        let block = self.client.get_block_by_hash_sync(header.hash)?;
        Self::process_block_pure(&block)
    }

    async fn get_chain_tip(&self) -> Result<BlockHeader, ChainSyncError> {
        let best_block = self.client.get_best_block_async().await?;
        let processed_block = Self::process_block_pure(&best_block)?;
        Ok(processed_block.header)
    }

    fn stream(
        &self,
        chain_tip_header: BlockHeader,
        last_header: Option<BlockHeader>,
    ) -> Pin<Box<dyn Stream<Item = FullBlock> + Send + 'static>> {
        let last_height = last_header.map_or(1, |h| h.height.0);
        let heights = last_height..=chain_tip_header.height.0;
        let client = Arc::clone(&self.client);
        tokio_stream::iter(heights)
            .map(move |height| {
                let client = Arc::clone(&client);
                async move {
                    client.get_block_by_height_retry_async(Height(height)).await.expect("Failed to fetch block by height")
                }
            }).buffer_unordered(self.fetching_par)
            .boxed()
    }
}
