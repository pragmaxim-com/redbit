use anyhow::Result;
use btc::block_provider::BtcBlockProvider;
use btc::model_v1::*;
use chain::launcher;
use chain::settings::AppConfig;
use std::collections::HashSet;

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::new("config/settings").expect("Failed to load app config");
    let (created, storage) = launcher::build_storage(&config).await?;

    assert_eq!(created, false, "We validate existing storage");

    let validated_height = 908244;
    let (btc_block, _) = BtcBlockProvider::block_from_file("huge", validated_height, 3713);
    let block_tx = Block::begin_read_ctx(&storage)?;
    let storage_block = Block::get(&block_tx, &Height(validated_height))?.expect("Block should exist in storage");

    let storage_tx_hashes: HashSet<TxHash> = storage_block.transactions.into_iter().map(|tx| tx.hash).collect();
    let btc_tx_hashes: HashSet<TxHash> = btc_block.txdata.into_iter().map(|tx| TxHash(*tx.compute_txid().as_ref())).collect();

    assert_eq!(storage_tx_hashes, btc_tx_hashes, "Transaction hashes in storage do not match those in the original block");

    Ok(())
}

