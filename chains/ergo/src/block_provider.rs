use crate::ergo_client::{ErgoCBOR, ErgoClient};
use crate::model_v1::{Address, Asset, AssetAction, AssetName, Block, BlockHash, BlockHeader, BlockPointer, Height, Timestamp, Transaction, TransactionPointer, TxHash, Utxo, UtxoPointer, Weight};
use crate::{model_v1, AssetType, ErgoConfig, ExplorerError};
use async_trait::async_trait;
use chain::api::{BlockProvider, ChainError};
use chain::monitor::BoxWeight;
use chain::settings::Parallelism;
use ergo_lib::chain::block::FullBlock;
use ergo_lib::chain::transaction::ergo_transaction::ErgoTransaction;
use ergo_lib::ergotree_ir::chain::address::AddressEncoder;
use ergo_lib::{
    ergotree_ir::{
        chain::{address, ergo_box::ErgoBox, token::TokenId},
        serialization::SigmaSerializable,
    },
    wallet::box_selector::ErgoBoxAssets,
};
use futures::stream::StreamExt;
use futures::Stream;
use redbit::redb::Durability;
use redbit::*;
use reqwest::Url;
use std::{pin::Pin, str::FromStr, sync::Arc};
use tokio_util::sync::CancellationToken;
use chain::chain_config;

pub struct ErgoBlockProvider {
    pub client: Arc<ErgoClient>,
    pub fetching_par: Parallelism,
}

impl ErgoBlockProvider {
    pub fn new() -> Result<Arc<Self>, ExplorerError> {
        let config: ErgoConfig = chain_config::load_config("config/ergo", "ERGO").expect("Failed to load Ergo configuration");
        let ergo_client = ErgoClient::new(Url::from_str(&config.api_host).unwrap(), config.api_key.clone())?;

        Ok(Arc::new(ErgoBlockProvider {
            client: Arc::new(ergo_client),
            fetching_par: config.fetching_parallelism.clone().into(),
        }))
    }

    pub fn process_block_pure(cbor: &ErgoCBOR) -> Result<Block, ChainError> {
        let b: FullBlock = serde_json::from_slice(&cbor.0).map_err(|e| ChainError::new(&format!("Failed to parse block CBOR: {}", e)))?;
        let mut block_weight: usize = 6;
        let mut result_txs = Vec::with_capacity(b.block_transactions.transactions.len());

        let block_hash: [u8; 32] = b.header.id.0.into();
        let prev_block_hash: [u8; 32] = b.header.parent_id.0.into();
        let height = Height(b.header.height);

        for (tx_index, tx) in b.block_transactions.transactions.iter().enumerate() {
            let tx_hash: [u8; 32] = tx.id().0.0;
            let tx_id = BlockPointer::from_parent(height, tx_index as u16);
            let (outs_weight, outputs) = Self::process_outputs(tx.outputs(), &tx_id);
            let mut inputs = Vec::with_capacity(tx.inputs.len());
            for input in &tx.inputs {
                inputs.push(model_v1::BoxId(input.box_id.as_ref().try_into().unwrap()));
            }
            block_weight += outs_weight + tx.inputs.len() + 1;
            result_txs.push(Transaction { id: tx_id, hash: TxHash(tx_hash), utxos: outputs, inputs: Vec::new(), input_refs: inputs, input_utxos: vec![] })
        }

        let header = BlockHeader {
            height,
            timestamp: Timestamp((b.header.timestamp / 1000) as u32),
            hash: BlockHash(block_hash),
            prev_hash: BlockHash(prev_block_hash),
            weight: Weight(block_weight as u32)
        };

        Ok(Block { height, header, transactions: result_txs })
    }

    fn process_outputs(outs: &[ErgoBox], tx_pointer: &BlockPointer) -> (BoxWeight, Vec<Utxo>) {
        let mut result_outs = Vec::with_capacity(outs.len());
        let mut asset_count = 0;
        for (out_index, out) in outs.iter().enumerate() {
            let box_id = model_v1::BoxId(out.box_id().as_ref().try_into().unwrap());
            let ergo_tree_template_opt = out.ergo_tree.template_bytes().ok();
            let address_opt = address::Address::recreate_from_ergo_tree(&out.ergo_tree).ok();
            let ergo_tree_opt = match &address_opt {
                Some(address::Address::P2S(sigma_bytes)) => Some(sigma_bytes.clone()),
                _ => out.ergo_tree.sigma_serialize_bytes().ok()
            };

            let address_bytes_opt =
                address_opt.map(|a| AddressEncoder::encode_address_as_bytes(crate::codec::MAINNET, &a));

            let address = Address(address_bytes_opt.unwrap_or_default());
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
                    let new_token_id: TokenId = outs.first().unwrap().box_id().into();
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
                box_id,
                tree,
                tree_template,
            })
        }
        (asset_count + result_outs.len(), result_outs)
    }
}

#[async_trait]
impl BlockProvider<ErgoCBOR, Block> for ErgoBlockProvider {

    fn block_processor(&self) -> Arc<dyn Fn(&ErgoCBOR) -> Result<Block, ChainError> + Send + Sync> {
        Arc::new(|raw| ErgoBlockProvider::process_block_pure(raw))
    }

    fn get_processed_block(&self, hash: BlockHash) -> Result<Option<Block>, ChainError> {
        match self.client.get_cbor_by_hash_sync(hash) {
            Ok(block) => {
                let processed_block = Self::process_block_pure(&block)?;
                Ok(Some(processed_block))
            },
            Err(_) => Ok(None),
        }
    }

    async fn get_chain_tip(&self) -> Result<BlockHeader, ChainError> {
        let best_block = self.client.get_best_block_async().await?;
        let processed_block = Self::process_block_pure(&best_block)?;
        Ok(processed_block.header)
    }

    fn stream(
        &self,
        remote_chain_tip_header: BlockHeader,
        last_persisted_header: Option<BlockHeader>,
        durability: Durability
    ) -> (Pin<Box<dyn Stream<Item = ErgoCBOR> + Send + 'static>>, CancellationToken) {
        let height_to_index_from = last_persisted_header.map_or(1, |h| h.height.0 + 1);
        let heights = height_to_index_from..=remote_chain_tip_header.height.0;
        let client = Arc::clone(&self.client);
        let s =
            tokio_stream::iter(heights).map(move |height| {
                let client = Arc::clone(&client);
                async move {
                    client.get_cbor_by_height_retry_async(Height(height)).await.expect("Failed to fetch block by height")
                }
            });
        
        let stream = match durability {
            Durability::None => s.buffer_unordered(self.fetching_par.0).boxed(),
            Durability::Immediate => s.buffered(self.fetching_par.0).boxed(),
            _ => unreachable!("Only None and Immediate durability modes are supported")
        };
        (stream, CancellationToken::new())
    }
}
