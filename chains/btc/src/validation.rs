use anyhow::Result;
use btc::config::BitcoinConfig;
use btc::model_v1::*;
use btc::rest_client::BtcClient;
use chain::launcher;
use chain::settings::AppConfig;
use std::collections::HashSet;

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::new("config/settings").expect("Failed to load app config");
    let (created, storage) = launcher::build_storage(&config).await?;

    assert_eq!(created, false, "We validate existing storage");

    let config = BitcoinConfig::new("config/btc").expect("Failed to load Bitcoin configuration");
    let client = Arc::new(BtcClient::new(&config)?);
    let block_tx = Block::begin_read_ctx(&storage)?;

    for height in 915500..915585 {
        let cbor = client.get_block_by_height(Height(height)).await?;
        let btc_block: bitcoin::Block = bitcoin::consensus::encode::deserialize(&cbor.raw)?;
        let storage_block = Block::get(&block_tx, &Height(height))?.expect("Block should exist in storage");
        let storage_tx_hashes: HashSet<TxHash> = storage_block.transactions.into_iter().map(|tx| tx.hash).collect();
        let btc_tx_hashes: HashSet<TxHash> = btc_block.txdata.into_iter().map(|tx| TxHash(*tx.compute_txid().as_ref())).collect();
        assert_eq!(storage_tx_hashes, btc_tx_hashes, "Transaction hashes in storage do not match those in the original block");
    }

    info!("Validation successful");
    Ok(())
}

