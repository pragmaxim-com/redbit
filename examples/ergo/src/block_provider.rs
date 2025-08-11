use crate::config::ErgoConfig;
use crate::ergo_client::ErgoClient;
use crate::model_v1;
use crate::model_v1::{
    Address, Asset, AssetAction, AssetName, UtxoPointer, AssetType, Block, BlockHash, BlockHeader, Height, BlockTimestamp, Transaction, TxHash,
    BlockPointer, Utxo, TransactionPointer,
};
use async_trait::async_trait;
use syncer::api::{BlockProvider, ChainSyncError};
use syncer::info;
use syncer::monitor::BoxWeight;
use ergo_lib::{chain::block::FullBlock, wallet::signing::ErgoTransaction};
use ergo_lib::{
    ergotree_ir::{
        chain::{address, ergo_box::ErgoBox, token::TokenId},
        serialization::SigmaSerializable,
    },
    wallet::box_selector::ErgoBoxAssets,
};
use futures::Stream;
use futures::stream::StreamExt;
use redbit::*;
use reqwest::Url;
use std::{pin::Pin, str::FromStr, sync::Arc};
use ergo_lib::ergotree_ir::chain::address::AddressEncoder;

pub struct ErgoBlockProvider {
    pub client: Arc<ErgoClient>,
    pub fetching_par: usize,
}

impl ErgoBlockProvider {
    pub fn new(ergo_config: &ErgoConfig, fetching_par: usize) -> Self {
        ErgoBlockProvider {
            client: Arc::new(ErgoClient { node_url: Url::from_str(&ergo_config.api_host).unwrap(), api_key: ergo_config.api_key.clone() }),
            fetching_par,
        }
    }
    fn process_outputs(&self, outs: &[ErgoBox], tx_pointer: BlockPointer) -> (BoxWeight, Vec<Utxo>) {
        let mut result_outs = Vec::with_capacity(outs.len());
        let mut asset_count = 0;
        for (out_index, out) in outs.iter().enumerate() {
            let box_id = out.box_id();
            let box_id_slice: &[u8] = box_id.as_ref();
            let box_id_bytes: Vec<u8> = box_id_slice.into();
            let ergo_tree_opt = out.ergo_tree.sigma_serialize_bytes().ok();
            let ergo_tree_template_opt = out.ergo_tree.template_bytes().ok();
            let address_opt =
                address::Address::recreate_from_ergo_tree(&out.ergo_tree)
                    .map(|a| AddressEncoder::encode_address_as_bytes(crate::codec::MAINNET, &a))
                    .ok();

            let address = Address(address_opt.unwrap_or_else(|| vec![]));
            let tree = model_v1::Tree(ergo_tree_opt.unwrap_or(vec![]));
            let tree_template = model_v1::TreeTemplate(ergo_tree_template_opt.unwrap_or(vec![]));

            let amount = *out.value.as_u64();
            let utxo_pointer = TransactionPointer::from_parent(tx_pointer.clone(), out_index as u16);

            let assets: Vec<Asset> = if let Some(assets) = out.tokens() {
                let mut result = Vec::with_capacity(assets.len());
                for (index, asset) in assets.enumerated() {
                    let asset_id: Vec<u8> = asset.token_id.into();
                    let amount = asset.amount;
                    let amount_u64: u64 = amount.into();
                    let is_mint = outs.first().is_some_and(|o| {
                        let new_token_id: TokenId = o.box_id().into();
                        new_token_id == asset.token_id
                    });

                    let action = match is_mint {
                        true => AssetType::Mint, // TODO!! for Minting it might not be enough to check first boxId
                        _ => AssetType::Transfer,
                    };
                    let asset_pointer = UtxoPointer::from_parent(utxo_pointer.clone(), index as u8);
                    result.push(Asset { id: asset_pointer, name: AssetName(asset_id), amount: amount_u64, asset_action: AssetAction(action.into()) });
                }
                result
            } else {
                vec![]
            };

            asset_count += assets.len();
            result_outs.push(Utxo {
                id: utxo_pointer.clone(),
                assets,
                address,
                amount,
                box_id: model_v1::BoxId(box_id_bytes.clone()),
                tree,
                tree_template,
            })
        }
        (asset_count + result_outs.len(), result_outs)
    }
}

#[async_trait]
impl BlockProvider<FullBlock, Block> for ErgoBlockProvider {
    fn process_block(&self, b: &FullBlock) -> Result<Block, ChainSyncError> {
        let mut block_weight: usize = 0;
        let mut result_txs = Vec::with_capacity(b.block_transactions.transactions.len());

        let block_hash: [u8; 32] = b.header.id.0.into();
        let prev_block_hash: [u8; 32] = b.header.parent_id.0.into();

        let id = Height(b.header.height);
        let header = BlockHeader {
            height: id.clone(),
            timestamp: BlockTimestamp((b.header.timestamp / 1000) as u32),
            hash: BlockHash(block_hash),
            prev_hash: BlockHash(prev_block_hash),
        };

        for (tx_index, tx) in b.block_transactions.transactions.iter().enumerate() {
            let tx_hash: [u8; 32] = tx.id().0.0;
            let tx_id = BlockPointer::from_parent(header.height.clone(), tx_index as u16);
            let (box_weight, outputs) = self.process_outputs(&tx.outputs().to_vec(), tx_id.clone()); //TODO perf check
            let inputs: Vec<model_v1::BoxId> = tx
                .inputs
                .iter()
                .map(|input| {
                    let box_id_slice: &[u8] = input.box_id.as_ref();
                    let box_id_bytes: Vec<u8> = box_id_slice.into();
                    model_v1::BoxId(box_id_bytes)
                })
                .collect();
            block_weight += box_weight;
            block_weight += tx.inputs.len();
            result_txs.push(Transaction { id: tx_id.clone(), hash: TxHash(tx_hash), utxos: outputs, inputs: vec![], transient_inputs: inputs })
        }

        Ok(Block { height: id.clone(), header, transactions: result_txs, weight: block_weight as u32 })
    }

    fn get_processed_block(&self, header: BlockHeader) -> Result<Block, ChainSyncError> {
        let block = self.client.get_block_by_hash_sync(header.hash)?;
        self.process_block(&block)
    }

    async fn get_chain_tip(&self) -> Result<BlockHeader, ChainSyncError> {
        let best_block = self.client.get_best_block_async().await?;
        let processed_block = self.process_block(&best_block)?;
        Ok(processed_block.header)
    }

    fn stream(
        &self,
        chain_tip_header: BlockHeader,
        last_header: Option<BlockHeader>,
    ) -> Pin<Box<dyn Stream<Item = FullBlock> + Send + 'static>> {
        let last_height = last_header.map_or(1, |h| h.height.0);
        info!("Indexing from {} to {}", last_height, chain_tip_header.height.0);
        let heights = last_height..=chain_tip_header.height.0;
        let client = Arc::clone(&self.client);
        tokio_stream::iter(heights)
            .map(move |height| {
                let client = Arc::clone(&client);
                tokio::task::spawn(async move { client.get_block_by_height_async(Height(height)).await.unwrap() })
            })
            .buffered(self.fetching_par)
            .map(|res| match res {
                Ok(block) => block,
                Err(e) => panic!("Error: {:?}", e), // lousy error handling
            })
            .boxed()
    }
}
