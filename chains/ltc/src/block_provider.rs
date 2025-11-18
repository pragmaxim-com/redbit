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
use litcoin::hashes::Hash;
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

    // Helper used in benches/tests to load canned blocks from JSON (same format as BTC benches)
    #[allow(dead_code)]
    pub fn block_from_file(size: &str, height: u32, tx_count: usize) -> (litcoin::Block, LtcCBOR) {
        use litcoin::Block;
        use litcoin::BlockHeader as BlockHeader;
        use std::str::FromStr;

        info!("Getting {} block with {} txs", size, tx_count);
        let path = format!("blocks/{}_block.json", size);
        let file_content = fs::read_to_string(path).expect("Failed to read block file");
        let v: serde_json::Value = serde_json::from_str(&file_content).expect("Failed to parse JSON");

        // Parse header fields from Litecoin Core-like JSON
        let version = v["version"].as_i64().unwrap() as i32;
        let prev_hash_s = v["previousblockhash"].as_str().unwrap();
        let merkle_root_s = v["merkleroot"].as_str().unwrap();
        let time = v["time"].as_u64().unwrap() as u32;
        let bits_hex = v["bits"].as_str().unwrap();
        let bits = u32::from_str_radix(bits_hex, 16).expect("invalid bits");
        let nonce = v["nonce"].as_u64().unwrap() as u32;

        let prev_hash = litcoin::BlockHash::from_str(prev_hash_s).expect("invalid prev hash");
        let merkle_root = litcoin::hash_types::TxMerkleNode::from_str(merkle_root_s).expect("invalid merkle root");

        let header = BlockHeader {
            version,
            prev_blockhash: prev_hash,
            merkle_root,
            time,
            bits,
            nonce,
        };

        // Transactions: each entry contains a hex-encoded tx
        let mut txs = Vec::new();
        if let Some(arr) = v["tx"].as_array() {
            for txv in arr {
                let tx_hex = txv["hex"].as_str().expect("missing tx hex");
                let tx_bytes = hex::decode(tx_hex).expect("invalid tx hex");
                let tx: litcoin::Transaction = litcoin::consensus::encode::deserialize(&tx_bytes).expect("tx deser");
                txs.push(tx);
            }
        }

        let block = Block { header, txdata: txs };
        let raw = litcoin::consensus::encode::serialize(&block);
        (block.clone(), LtcCBOR { height: Height(height), raw })
    }

    pub fn process_block_pure(cbor: &LtcCBOR) -> Result<Block, ChainError> {
        let block: litcoin::Block = litcoin::consensus::encode::deserialize(&cbor.raw).map_err(|e| ChainError::new(&format!("Failed to deser CBOR: {}", e)))?;
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
            hash: BlockHash(block.block_hash().as_hash().into_inner()),
            prev_hash: BlockHash(block.header.prev_blockhash.as_hash().into_inner()),
            merkle_root: MerkleRoot(block.header.merkle_root.as_hash().into_inner()),
            weight: Weight(block_weight as u32),
        };

        Ok(Block { height, header, transactions })
    }

    fn process_inputs(ins: &[litcoin::TxIn]) -> Vec<InputRef> {
        ins.iter()
            .map(|input| {
                let tx_hash = TxHash(input.previous_output.txid.as_hash().into_inner());
                InputRef { tx_hash, index: input.previous_output.vout as u16 }
            })
            .collect()
    }

    /// Litecoin scriptPubKey to address bytes/tag.
    ///
    /// Legacy version bytes differ from Bitcoin:
    /// - P2PKH: 0x30
    /// - P2SH:  0x32
    pub fn spk_to_address_bytes_or_tag(spk: &litcoin::Script) -> Vec<u8> {
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
            if let Some(wv) = spk.witness_version() {
                let ver = wv.into_num() as u8;
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

    fn process_outputs(outs: &[litcoin::TxOut], tx_pointer: BlockPointer) -> (BoxWeight, Vec<Utxo>) {
        let mut result_outs = Vec::with_capacity(outs.len());
        for (out_index, out) in outs.iter().enumerate() {
            let script = &out.script_pubkey;
            let address = Self::spk_to_address_bytes_or_tag(script);
            result_outs.push(Utxo {
                id: TransactionPointer::from_parent(tx_pointer, out_index as u16),
                amount: out.value.into(),
                script_hash: ScriptHash(out.script_pubkey.as_bytes().to_vec()),
                address: Address(address),
            })
        }
        (result_outs.len(), result_outs)
    }

    fn process_tx(height: Height, tx_index: u16, tx: &litcoin::Transaction) -> Transaction {
        let tx_pointer = BlockPointer::from_parent(height, tx_index);
        let (_, outputs) = Self::process_outputs(&tx.output, tx_pointer);
        Transaction {
            id: tx_pointer,
            hash: TxHash(tx.txid().as_hash().into_inner()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use litcoin::blockdata::script::Script;
    use litcoin::hash_types::{PubkeyHash, ScriptHash as LScriptHash, WPubkeyHash, WScriptHash};

    #[test]
    fn p2pkh_tag_matches_litecoin_version_byte() {
        let h = PubkeyHash::from_slice(&[0x11; 20]).unwrap();
        let spk = Script::new_p2pkh(&h);
        let out = LtcBlockProvider::spk_to_address_bytes_or_tag(&spk);
        assert_eq!(out[0], 0x30);
        assert_eq!(&out[1..], &h[..]);
    }

    #[test]
    fn p2sh_tag_matches_litecoin_version_byte() {
        let h = LScriptHash::from_slice(&[0x22; 20]).unwrap();
        let spk = Script::new_p2sh(&h);
        let out = LtcBlockProvider::spk_to_address_bytes_or_tag(&spk);
        assert_eq!(out[0], 0x32);
        assert_eq!(&out[1..], &h[..]);
    }

    #[test]
    fn segwit_v0_20_and_32() {
        let wpkh = WPubkeyHash::from_slice(&[0x33; 20]).unwrap();
        let spk20 = Script::new_v0_p2wpkh(&wpkh);
        let out20 = LtcBlockProvider::spk_to_address_bytes_or_tag(&spk20);
        assert_eq!(out20[0], TAG_SEGWIT);
        assert_eq!(out20[1], 0);
        assert_eq!(&out20[2..], &wpkh[..]);

        let wsh = WScriptHash::from_slice(&[0x44; 32]).unwrap();
        let spk32 = Script::new_v0_p2wsh(&wsh);
        let out32 = LtcBlockProvider::spk_to_address_bytes_or_tag(&spk32);
        assert_eq!(out32[0], TAG_SEGWIT);
        assert_eq!(out32[1], 0);
        assert_eq!(&out32[2..], &wsh[..]);
    }

    #[test]
    fn op_return_and_non_addr() {
        let op = Script::new_op_return(&[1,2,3]);
        assert_eq!(LtcBlockProvider::spk_to_address_bytes_or_tag(&op), vec![TAG_OP_RETURN]);

        let empty = Script::new();
        assert_eq!(LtcBlockProvider::spk_to_address_bytes_or_tag(&empty), vec![TAG_NON_ADDR]);
    }
}
