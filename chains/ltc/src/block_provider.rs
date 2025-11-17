use crate::codec::{TAG_NON_ADDR, TAG_OP_RETURN, TAG_SEGWIT};
use crate::model_v1::{Address, Block, BlockHash, BlockPointer, Header, Height, InputRef, MerkleRoot, ScriptHash, Timestamp, Transaction, TransactionPointer, TxHash, Utxo, Weight};
use crate::rest_client::{LtcCBOR, LtcClient};
use crate::LitecoinConfig;
use async_trait::async_trait;
use chain::api::BlockProvider;
use chain::block_stream::{BlockStream, RestBlockStream};
use chain::chain_config;
use chain::monitor::BoxWeight;
use chain::settings::{AppConfig, Parallelism};
use redbit::info;
use redbit::*;
use serde_json;
use std::{fs, sync::Arc};
use tokio::sync::mpsc::Receiver;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use chain::err::ChainError;

pub struct LtcBlockProvider {
    pub client: Arc<LtcClient>,
    pub block_stream: Arc<dyn BlockStream<LtcCBOR, Block>>,
}

impl LtcBlockProvider {
    pub fn new(config: AppConfig) -> Result<Arc<dyn BlockProvider<LtcCBOR, Block>>, ChainError> {
        let ltc_config: LitecoinConfig = chain_config::load_config("config/ltc", "LITECOIN").expect("Failed to load Litecoin configuration");
        let client = Arc::new(LtcClient::new(&ltc_config)?);
        let fetching_par: Parallelism = ltc_config.fetching_parallelism.clone();
        let max_entity_buffer_kb_size = config.indexer.max_entity_buffer_kb_size;
        let block_stream = Arc::new(RestBlockStream::new(Arc::clone(&client), fetching_par.clone(), max_entity_buffer_kb_size));
        Ok(Arc::new(LtcBlockProvider { client, block_stream }))
    }

    // Optional helper only used in benches/tests; disable to avoid serde bounds on bitcoin::Block.
    // #[allow(dead_code)]
    // pub fn block_from_file(size: &str, height: u32, tx_count: usize) -> (bitcoin::Block, LtcCBOR) {
    //     info!("Getting {} block with {} txs", size, tx_count);
    //     let path = format!("blocks/{}_block.json", size);
    //     let file_content = fs::read_to_string(path).expect("Failed to read block file");
    //     let block: bitcoin::Block = serde_json::from_str(&file_content).expect("Failed to deserialize block from JSON");
    //     (block.clone(), LtcCBOR { height: Height(height), raw: bitcoin::consensus::encode::serialize(&block) })
    // }

    pub fn process_block_pure(cbor: &LtcCBOR) -> Result<Block, ChainError> {
        let block: bitcoin::Block = bitcoin::consensus::encode::deserialize(&cbor.raw).map_err(|e| ChainError::new(&format!("Failed to deser CBOR: {}", e)))?;
        let height = cbor.height;
        let mut block_weight = 6;
        let transactions = block
            .txdata
            .iter()
            .enumerate()
            .map(|(tx_index, tx)| {
                block_weight += tx.input.len() + tx.output.len() + 1;
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

        Ok(Block { height, header, transactions })
    }

    fn process_inputs(ins: &[bitcoin::TxIn]) -> Vec<InputRef> {
        ins.iter()
            .map(|input| {
                let tx_hash = TxHash(*input.previous_output.txid.as_ref());
                InputRef { tx_hash, index: input.previous_output.vout as u16 }
            })
            .collect()
    }

    /// Litecoin scriptPubKey to address bytes/tag.
    ///
    /// Legacy version bytes differ from Bitcoin:
    /// - P2PKH: 0x30
    /// - P2SH:  0x32
    pub fn spk_to_address_bytes_or_tag(spk: &bitcoin::Script) -> Vec<u8> {
        use bitcoin::blockdata::script::witness_version::WitnessVersion;
        let b = spk.as_bytes();

        // Legacy P2PKH
        if spk.is_p2pkh() {
            debug_assert!(b.len() >= 25);
            let h = &b[3..23]; // 20 bytes
            let mut v = Vec::with_capacity(21);
            v.push(0x30);
            v.extend_from_slice(h);
            return v;
        }
        // Legacy P2SH
        if spk.is_p2sh() {
            debug_assert!(b.len() >= 23);
            let h = &b[2..22]; // 20 bytes
            let mut v = Vec::with_capacity(21);
            v.push(0x32);
            v.extend_from_slice(h);
            return v;
        }
        // Segwit
        if spk.is_witness_program() {
            let opcode = spk.first_opcode().expect("witness_program => len > 4");
            if let Ok(wv) = WitnessVersion::try_from(opcode) {
                let ver  = wv.to_num() as u8;
                let prog = &b[2..];
                if prog.len() == 20 || prog.len() == 32 {
                    let mut v = Vec::with_capacity(2 + prog.len());
                    v.push(TAG_SEGWIT);
                    v.push(ver);
                    v.extend_from_slice(prog);
                    return v;
                }
            }
            return vec![TAG_NON_ADDR];
        }
        if spk.is_op_return() {
            return vec![TAG_OP_RETURN];
        }
        vec![TAG_NON_ADDR]
    }

    fn process_outputs(outs: &[bitcoin::TxOut], tx_pointer: BlockPointer) -> (BoxWeight, Vec<Utxo>) {
        let mut result_outs = Vec::with_capacity(outs.len());
        for (out_index, out) in outs.iter().enumerate() {
            let script = out.script_pubkey.as_script();
            let address = Self::spk_to_address_bytes_or_tag(script);
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
            input_refs: Self::process_inputs(&tx.input),
            input_utxos: vec![],
        }
    }
}

#[async_trait]
impl BlockProvider<LtcCBOR, Block> for LtcBlockProvider {
    fn block_processor(&self) -> Arc<dyn Fn(&LtcCBOR) -> Result<Block, ChainError> + Send + Sync> {
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

    fn block_stream(
        &self,
        remote_chain_tip_header: Header,
        last_persisted_header: Option<Header>,
        shutdown: watch::Receiver<bool>,
        batch: bool,
    ) -> (Receiver<Vec<LtcCBOR>>, JoinHandle<()>) {
        self.block_stream.stream(remote_chain_tip_header, last_persisted_header, shutdown, batch)
    }
}
