use crate::btc_client::{BtcBlock, BtcClient};
use crate::config::BitcoinConfig;
use crate::model_v1::{Address, Block, BlockHash, Header, BlockPointer, BlockTimestamp, ExplorerError, Height, MerkleRoot, ScriptHash, TempInputRef, Transaction, TransactionPointer, TxHash, Utxo, Weight};
use async_trait::async_trait;
use futures::stream::StreamExt;
use futures::Stream;
use redbit::*;
use std::{pin::Pin, sync::Arc};
use syncer::api::{BlockProvider, ChainSyncError};
use syncer::monitor::BoxWeight;

pub const SENTINEL: [u8; 25] = [
    0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0xD3, 0x0A, 0x40, 0x06,
];

pub struct BtcBlockProvider {
    pub client: Arc<BtcClient>,
    pub fetching_par: usize,
}

impl BtcBlockProvider {
    pub fn new() -> Result<Arc<Self>, ExplorerError> {
        let config = BitcoinConfig::new("config/btc").expect("Failed to load Bitcoin configuration");
        let client = Arc::new(BtcClient::new(&config)?);
        let fetching_par: usize = config.fetching_parallelism.clone().into();
        Ok(Arc::new(BtcBlockProvider { client, fetching_par }))
    }

    pub fn process_block_pure(block: &BtcBlock) -> Result<Block, ChainSyncError> {
        let mut block_weight = 0;
        let transactions = block
            .underlying
            .txdata
            .iter()
            .enumerate()
            .map(|(tx_index, tx)| {
                block_weight += tx.input.len() + tx.output.len();
                Self::process_tx(block.height, tx_index as u16, &tx)
            })
            .collect();
        
        let header = Header {
            height: block.height,
            timestamp: BlockTimestamp(block.underlying.header.time),
            hash: BlockHash(*block.underlying.block_hash().as_ref()),
            prev_hash: BlockHash(*block.underlying.header.prev_blockhash.as_ref()),
            merkle_root: MerkleRoot(*block.underlying.header.merkle_root.as_ref()),
            weight: Weight(block_weight as u32),
        };

        Ok(Block {
            height: block.height,
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
            transient_inputs: Self::process_inputs(&tx.input),
        }
    }
}

#[async_trait]
impl BlockProvider<BtcBlock, Block> for BtcBlockProvider {
    fn block_processor(&self) -> Arc<dyn Fn(&BtcBlock) -> Result<Block, ChainSyncError> + Send + Sync> {
        Arc::new(|raw| Self::process_block_pure(raw))
    }

    fn get_processed_block(&self, header: Header) -> Result<Block, ChainSyncError> {
        let block = self.client.get_block_by_hash(header.hash)?;
        Self::process_block_pure(&block)
    }

    async fn get_chain_tip(&self) -> Result<Header, ChainSyncError> {
        let best_block = self.client.get_best_block()?;
        let processed_block = Self::process_block_pure(&best_block)?;
        Ok(processed_block.header)
    }

    fn stream(
        &self,
        chain_tip_header: Header,
        last_header: Option<Header>,
    ) -> Pin<Box<dyn Stream<Item = BtcBlock> + Send + 'static>> {
        let last_height = last_header.map_or(0, |h| h.height.0);
        let heights = last_height..=chain_tip_header.height.0;
        let client = Arc::clone(&self.client);
        tokio_stream::iter(heights)
            .map(move |height| {
                let client = Arc::clone(&client);
                tokio::task::spawn_blocking(move || client.get_block_by_height(Height(height)).expect("Failed to get block by height"))
            })
            .buffer_unordered(self.fetching_par)
            .map(|res| match res {
                Ok(block) => block,
                Err(e) => panic!("Error: {:?}", e), // TODO lousy error handling
            })
            .boxed()
    }
}
