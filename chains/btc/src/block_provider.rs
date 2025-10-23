use crate::codec::{TAG_NON_ADDR, TAG_OP_RETURN, TAG_SEGWIT};
use crate::model_v1::{Address, Block, BlockHash, BlockPointer, Header, Height, InputRef, MerkleRoot, ScriptHash, Timestamp, Transaction, TransactionPointer, TxHash, Utxo, Weight};
use crate::rest_client::{BtcCBOR, BtcClient};
use crate::BitcoinConfig;
use async_trait::async_trait;
use chain::api::{BlockProvider, ChainError};
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

pub struct BtcBlockProvider {
    pub client: Arc<BtcClient>,
    pub block_stream: Arc<dyn BlockStream<BtcCBOR, Block>>,
}

impl BtcBlockProvider {
    pub fn new(config: AppConfig) -> Result<Arc<dyn BlockProvider<BtcCBOR, Block>>, ChainError> {
        let btc_config: BitcoinConfig = chain_config::load_config("config/btc", "BITCOIN").expect("Failed to load Bitcoin configuration");
        let client = Arc::new(BtcClient::new(&btc_config)?);
        let fetching_par: Parallelism = btc_config.fetching_parallelism.clone();
        let max_entity_buffer_kb_size = config.indexer.max_entity_buffer_kb_size;
        let block_stream = Arc::new(RestBlockStream::new(Arc::clone(&client), fetching_par.clone(), max_entity_buffer_kb_size));
        Ok(Arc::new(BtcBlockProvider { client, block_stream }))
    }

    pub fn block_from_file(size: &str, height: u32, tx_count: usize) -> (bitcoin::Block, BtcCBOR) {
        info!("Getting {} block with {} txs", size, tx_count);
        let path = format!("blocks/{}_block.json", size);
        let file_content = fs::read_to_string(path).expect("Failed to read block file");
        let block: bitcoin::Block = serde_json::from_str(&file_content).expect("Failed to deserialize block from JSON");
        (block.clone(), BtcCBOR { height: Height(height), raw: bitcoin::consensus::encode::serialize(&block) })
    }


    pub fn process_block_pure(cbor: &BtcCBOR) -> Result<Block, ChainError> {
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

        Ok(Block {
            height,
            header,
            transactions,
        })
    }

    fn process_inputs(ins: &[bitcoin::TxIn]) -> Vec<InputRef> {
        ins.iter()
            .map(|input| {
                let tx_hash = TxHash(*input.previous_output.txid.as_ref());
                InputRef { tx_hash, index: input.previous_output.vout }
            })
            .collect()
    }

    /// Convert a scriptPubKey to your index key bytes (or a tag).
    ///
    /// Output formats:
    /// - Legacy P2PKH: [0x00 || hash160(pubkey)]            (21 bytes)
    /// - Legacy P2SH : [0x05 || hash160(redeem_script)]     (21 bytes)
    /// - Segwit      : [TAG_SEGWIT, ver(0..=16), program]   (22 or 34 bytes)
    /// - OP_RETURN   : [TAG_OP_RETURN]
    /// - Other       : [TAG_NON_ADDR]
    ///
    /// Complexity: O(1) control + one O(n) copy where n ∈ {20,32}.
    pub fn spk_to_address_bytes_or_tag(spk: &bitcoin::Script) -> Vec<u8> {
        use bitcoin::blockdata::script::witness_version::WitnessVersion;
        // Single load of script bytes; all branches reuse it.
        let b = spk.as_bytes();

        // Legacy P2PKH: OP_DUP OP_HASH160 <20> <hash> OP_EQUALVERIFY OP_CHECKSIG
        if spk.is_p2pkh() {
            debug_assert!(b.len() >= 25);
            let h = &b[3..23]; // 20 bytes
            let mut v = Vec::with_capacity(21);
            v.push(0x00);
            v.extend_from_slice(h);
            return v;
        }

        // Legacy P2SH: OP_HASH160 <20> <hash> OP_EQUAL
        if spk.is_p2sh() {
            debug_assert!(b.len() >= 23);
            let h = &b[2..22]; // 20 bytes
            let mut v = Vec::with_capacity(21);
            v.push(0x05);
            v.extend_from_slice(h);
            return v;
        }

        // Segwit: OP_{ver} <pushlen> <program>
        if spk.is_witness_program() {
            // Same version parsing as the library.
            // (is_witness_program() guarantees len > 4)
            let opcode = spk.first_opcode().expect("witness_program => len > 4");
            if let Ok(wv) = WitnessVersion::try_from(opcode) {
                let ver  = wv.to_num() as u8;
                let prog = &b[2..]; // witness program bytes (not including ver/pushlen)
                if prog.len() == 20 || prog.len() == 32 {
                    let mut v = Vec::with_capacity(2 + prog.len());
                    v.push(TAG_SEGWIT);
                    v.push(ver);
                    v.extend_from_slice(prog);
                    return v;
                }
            }
            // Invalid version or program length: we don’t index it as an address.
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

    fn block_stream(
        &self,
        remote_chain_tip_header: Header,
        last_persisted_header: Option<Header>,
        shutdown: watch::Receiver<bool>,
        batch: bool,
    ) -> (Receiver<Vec<BtcCBOR>>, JoinHandle<()>) {
        self.block_stream.stream(remote_chain_tip_header, last_persisted_header, shutdown, batch)
    }
}

#[cfg(test)]
mod btc_address_tests {
    use super::*;
    use crate::codec::{BaseOrBech, TAG_NON_ADDR, TAG_OP_RETURN, TAG_SEGWIT};
    use crate::model_v1::serde_json;
    use bitcoin::address::AddressType;
    use bitcoin::blockdata::script::Builder;
    use bitcoin::script::PushBytesBuf;
    use bitcoin::{Address, Network};
    use serde::{Deserialize, Serialize};
    use serde_with::serde_as;
    use std::str::FromStr;

    // Wrapper to use your serde for text<->bytes checks.
    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct BtcWrap(#[serde_as(as = "BaseOrBech")] Vec<u8>);

    // Known-good mainnet vectors (from the `bitcoin` crate / BIPs).
    const P2PKH_S: &str = "1QJVDzdqb1VpbDK7uDeyVXy9mR27CJiyhY";
    const P2SH_S:  &str = "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy";
    const WPKH_S:  &str = "bc1qvzvkjn4q3nszqxrv3nraga2r822xjty3ykvkuw"; // v0, 20
    const WSH_S:   &str = "bc1qwqdg6squsna38e46795at95yu9atm8azzmyvckulcc7kytlcckxswvvzej"; // v0, 32
    const TR_S:    &str = "bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr"; // v1, 32

    fn addr(s: &str) -> Address {
        Address::from_str(s).unwrap().require_network(Network::Bitcoin).unwrap()
    }

    #[test]
    fn p2pkh_layout() {
        let a = addr(P2PKH_S);
        assert_eq!(a.address_type(), Some(AddressType::P2pkh));
        let spk = a.script_pubkey();
        let b = BtcBlockProvider::spk_to_address_bytes_or_tag(&spk);
        assert_eq!(b.len(), 21);
        assert_eq!(b[0], 0x00);
    }

    #[test]
    fn p2sh_layout() {
        let a = addr(P2SH_S);
        assert_eq!(a.address_type(), Some(AddressType::P2sh));
        let spk = a.script_pubkey();
        let b = BtcBlockProvider::spk_to_address_bytes_or_tag(&spk);
        assert_eq!(b.len(), 21);
        assert_eq!(b[0], 0x05);
    }

    #[test]
    fn wpkh_layout_v0_20() {
        let a = addr(WPKH_S);
        assert_eq!(a.address_type(), Some(AddressType::P2wpkh));
        let spk = a.script_pubkey();
        let b = BtcBlockProvider::spk_to_address_bytes_or_tag(&spk);
        assert_eq!(b.len(), 22);
        assert_eq!(b[0], TAG_SEGWIT);
        assert_eq!(b[1], 0); // v0
    }

    #[test]
    fn wsh_layout_v0_32() {
        let a = addr(WSH_S);
        assert_eq!(a.address_type(), Some(AddressType::P2wsh));
        let spk = a.script_pubkey();
        let b = BtcBlockProvider::spk_to_address_bytes_or_tag(&spk);
        assert_eq!(b.len(), 34);
        assert_eq!(b[0], TAG_SEGWIT);
        assert_eq!(b[1], 0); // v0
    }

    #[test]
    fn taproot_layout_v1_32() {
        let a = addr(TR_S);
        assert_eq!(a.address_type(), Some(AddressType::P2tr));
        let spk = a.script_pubkey();
        let b = BtcBlockProvider::spk_to_address_bytes_or_tag(&spk);
        assert_eq!(b.len(), 34);
        assert_eq!(b[0], TAG_SEGWIT);
        assert_eq!(b[1], 1); // v1
    }

    #[test]
    fn serde_parity_for_valid_scripts() {
        // For valid scripts, index bytes must equal what your serde decodes from the string,
        // and serialize back to the same string.
        for s in [P2PKH_S, P2SH_S, WPKH_S, WSH_S, TR_S] {
            let a = addr(s);
            let spk = a.script_pubkey();

            // index bytes from script
            let idx = BtcBlockProvider::spk_to_address_bytes_or_tag(&spk);

            // text -> bytes via serde must match index bytes
            let parsed: BtcWrap = serde_json::from_str(&format!("\"{s}\"")).unwrap();
            assert_eq!(parsed.0, idx, "index bytes mismatch for {s}");

            // bytes -> text back must be canonical
            let back = serde_json::to_string(&parsed).unwrap();
            assert_eq!(back, format!("\"{s}\""));
        }
    }

    #[test]
    fn op_return_and_non_addr_tags() {
        // OP_RETURN <data>
        let opret = Builder::new()
            .push_opcode(bitcoin::opcodes::all::OP_RETURN)
            .push_slice(&[1, 2, 3])
            .into_script();
        assert_eq!(BtcBlockProvider::spk_to_address_bytes_or_tag(&opret), vec![TAG_OP_RETURN]);

        // Junk (not an address form)
        let junk = Builder::new().push_int(42).push_slice(&[9,9,9,9]).into_script();
        assert_eq!(BtcBlockProvider::spk_to_address_bytes_or_tag(&junk), vec![TAG_NON_ADDR]);
    }

    #[test]
    fn invalid_witness_program_lengths_become_non_addr() {
        // Build witness-looking scripts with invalid program lengths without invoking checked constructors.

        // v0 + 21 bytes (invalid for v0; only 20 or 32 are valid)
        let prog_21 = vec![0u8; 21];
        let pb21 = PushBytesBuf::try_from(prog_21).unwrap();
        let s21 = Builder::new().push_int(0).push_slice(&pb21).into_script();
        assert!(s21.is_witness_program()); // syntactically looks like one
        assert_eq!(BtcBlockProvider::spk_to_address_bytes_or_tag(&s21), vec![TAG_NON_ADDR]);

        // v1 + 31 bytes (invalid for v1; should be 32)
        let prog_31 = vec![0u8; 31];
        let pb31 = PushBytesBuf::try_from(prog_31).unwrap();
        let s31 = Builder::new().push_int(1).push_slice(&pb31).into_script();
        assert!(s31.is_witness_program());
        assert_eq!(BtcBlockProvider::spk_to_address_bytes_or_tag(&s31), vec![TAG_NON_ADDR]);
    }
}
