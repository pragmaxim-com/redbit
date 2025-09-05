use crate::config::BitcoinConfig;
use crate::model_v1::{Address, Block, BlockHash, BlockPointer, Timestamp, Header, Height, MerkleRoot, ScriptHash, TempInputRef, Transaction, TransactionPointer, TxHash, Utxo, Weight};
use async_trait::async_trait;
use chain::api::{BlockProvider, ChainError};
use chain::batcher::SyncMode;
use chain::monitor::BoxWeight;
use futures::stream::StreamExt;
use futures::Stream;
use redbit::*;
use std::{pin::Pin, sync::Arc};
use chain::settings::Parallelism;
use crate::ExplorerError;
use crate::rest_client::{BtcCBOR, BtcClient};

pub const SENTINEL: [u8; 25] = [
    0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0xD3, 0x0A, 0x40, 0x06,
];

pub struct BtcBlockProvider {
    pub client: Arc<BtcClient>,
    pub fetching_par: Parallelism,
}

impl BtcBlockProvider {
    pub fn new() -> Result<Arc<Self>, ExplorerError> {
        let config = BitcoinConfig::new("config/btc").expect("Failed to load Bitcoin configuration");
        let client = Arc::new(BtcClient::new(&config)?);
        let fetching_par: Parallelism = config.fetching_parallelism.clone();
        Ok(Arc::new(BtcBlockProvider { client, fetching_par }))
    }

    pub fn process_block_pure(cbor: &BtcCBOR) -> Result<Block, ChainError> {
        let block: bitcoin::Block = bitcoin::consensus::encode::deserialize(&cbor.raw).map_err(|e| ChainError::new(&format!("Failed to deser CBOR: {}", e)))?;
        let height = cbor.height;
        let mut block_weight = 0;
        let transactions = block
            .txdata
            .iter()
            .enumerate()
            .map(|(tx_index, tx)| {
                block_weight += tx.input.len() + tx.output.len();
                Self::process_tx(height, tx_index as u16, &tx)
            })
            .collect();
        
        let header = Header {
            height: height.clone(),
            timestamp: Timestamp(block.header.time),
            hash: BlockHash(*block.block_hash().as_ref()),
            prev_hash: BlockHash(*block.header.prev_blockhash.as_ref()),
            merkle_root: MerkleRoot(*block.header.merkle_root.as_ref()),
            weight: Weight(block_weight as u32),
        };

        Ok(Block {
            height,
            header,
            transactions,
        })
    }

    fn process_inputs(ins: &[bitcoin::TxIn]) -> Vec<TempInputRef> {
        ins.iter()
            .map(|input| {
                let tx_hash = TxHash(*input.previous_output.txid.as_ref());
                TempInputRef { tx_hash, index: input.previous_output.vout }
            })
            .collect()
    }
    fn process_outputs(outs: &[bitcoin::TxOut], tx_pointer: BlockPointer) -> (BoxWeight, Vec<Utxo>) {
        let mut result_outs = Vec::with_capacity(outs.len());
        for (out_index, out) in outs.iter().enumerate() {
            let address = if let Ok(address) = bitcoin::Address::from_script(out.script_pubkey.as_script(), bitcoin::Network::Bitcoin) {
                address.to_string().into_bytes()
            } else {
                SENTINEL.to_vec()
            };
            result_outs.push(Utxo {
                id: TransactionPointer::from_parent(tx_pointer, out_index as u16),
                amount: out.value.to_sat().into(),
                script_hash: ScriptHash(out.script_pubkey.as_bytes().to_vec()),
                address: Address(address),
            })
        }
        (result_outs.len(), result_outs)
    }
    fn process_tx(height: Height, tx_index: u16, tx: &bitcoin::Transaction) -> Transaction {
        let tx_pointer = BlockPointer::from_parent(height, tx_index);
        let (_, outputs) = Self::process_outputs(&tx.output, tx_pointer);
        Transaction {
            id: tx_pointer,
            hash: TxHash(*tx.compute_txid().as_ref()),
            utxos: outputs,
            inputs: vec![],
            temp_input_refs: Self::process_inputs(&tx.input),
        }
    }
}

#[async_trait]
impl BlockProvider<BtcCBOR, Block> for BtcBlockProvider {
    fn block_processor(&self) -> Arc<dyn Fn(&BtcCBOR) -> Result<Block, ChainError> + Send + Sync> {
        Arc::new(|raw| Self::process_block_pure(raw))
    }

    fn get_processed_block(&self, hash: BlockHash) -> Result<Option<Block>, ChainError> {
        match self.client.get_block_by_hash_str_sync(hash) {
            Ok(block) => {
                let processed_block = Self::process_block_pure(&block)?;
                Ok(Some(processed_block))
            },
            Err(_) => Ok(None),
        }
    }

    async fn get_chain_tip(&self) -> Result<Header, ChainError> {
        let best_block = self.client.get_best_block().await?;
        let processed_block = Self::process_block_pure(&best_block)?;
        Ok(processed_block.header)
    }

    fn stream(
        &self,
        remote_chain_tip_header: Header,
        last_persisted_header: Option<Header>,
        mode: SyncMode
    ) -> Pin<Box<dyn Stream<Item = BtcCBOR> + Send + 'static>> {
        let height_to_index_from = last_persisted_header.map_or(1, |h| h.height.0 + 1);
        let heights = height_to_index_from..=remote_chain_tip_header.height.0;
        let client = Arc::clone(&self.client);
        let s =
            tokio_stream::iter(heights).map(move |height| {
                let client = Arc::clone(&client);
                async move {
                    client.get_block_by_height(Height(height)).await.expect("Failed to fetch block by height")
                }
            });
        match mode {
            SyncMode::Batching => s.buffer_unordered(self.fetching_par.0).boxed(),
            SyncMode::Continuous => s.buffered(self.fetching_par.0).boxed(),
        }
    }
}
