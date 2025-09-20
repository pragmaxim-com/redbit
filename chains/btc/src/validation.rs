use crate::model_v1::{Block, Height, TxHash};
use crate::rest_client::BtcCBOR;
use anyhow::Result;
use chain::launcher;
use chain::settings::AppConfig;
use redbit::info;
use std::collections::HashSet;
use std::fs;

pub fn block_from_file(size: &str, height: u32, tx_count: usize) -> (bitcoin::Block, BtcCBOR) {
    info!("Getting {} block with {} txs", size, tx_count);
    let path = format!("blocks/{}_block.json", size);
    let file_content = fs::read_to_string(path).expect("Failed to read block file");
    let block: bitcoin::Block = serde_json::from_str(&file_content).expect("Failed to deserialize block from JSON");
    (block.clone(), BtcCBOR { height: Height(height), raw: bitcoin::consensus::encode::serialize(&block) })
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::new("config/settings").expect("Failed to load app config");
    let (created, storage) = launcher::build_storage(&config).await?;

    assert_eq!(created, false, "We validate existing storage");

    let validated_height = 908244;
    let (btc_block, _) = block_from_file("huge", validated_height, 3713);
    let block_tx = Block::begin_read_ctx(&storage)?;
    let storage_block = Block::get(&block_tx, &Height(validated_height))?.expect("Block should exist in storage");

    let storage_tx_hashes: HashSet<TxHash> = storage_block.transactions.into_iter().map(|tx| tx.hash).collect();
    let btc_tx_hashes: HashSet<TxHash> = btc_block.txdata.into_iter().map(|tx| TxHash(*tx.compute_txid().as_ref())).collect();

    assert_eq!(storage_tx_hashes, btc_tx_hashes, "Transaction hashes in storage do not match those in the original block");

    Ok(())
}

