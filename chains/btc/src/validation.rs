use anyhow::Result;
use btc::config::BitcoinConfig;
use btc::model_v1::*;
use btc::rest_client::BtcClient;
use chain::launcher;
use chain::settings::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::new("config/settings").expect("Failed to load app config");
    let (created, storage_owner, storage) = launcher::build_storage(&config).await?;

    assert_eq!(created, false, "We validate existing storage");

    let config = BitcoinConfig::new("config/btc").expect("Failed to load Bitcoin configuration");
    let client = Arc::new(BtcClient::new(&config)?);
    let block_tx = Block::begin_read_ctx(&storage)?;

    for height in 915000..915585 {
        let cbor = client.get_block_by_height(Height(height)).await?;
        let btc_block: bitcoin::Block = bitcoin::consensus::encode::deserialize(&cbor.raw)?;
        let storage_block = Block::get(&block_tx, &Height(height))?.expect("Block should exist in storage");
        let storage_tx_hashes: Vec<TxHash> = storage_block.transactions.iter().map(|tx| tx.hash).collect();
        let btc_tx_hashes: Vec<TxHash> = btc_block.txdata.iter().map(|tx| TxHash(*tx.compute_txid().as_ref())).collect();
        assert_eq!(storage_tx_hashes, btc_tx_hashes, "Transaction hashes in storage do not match those in the original block");

        let storage_scripts: Vec<ScriptHash> =
            storage_block.transactions.iter()
                .flat_map(|tx| tx.utxos.iter().map(|out|out.script_hash.clone()))
                .collect();

        let btc_scripts: Vec<ScriptHash> =
            btc_block.txdata.iter()
                .flat_map(|tx| tx.output.iter().map(|out|ScriptHash(out.script_pubkey.as_bytes().to_vec())))
                .collect();

        assert_eq!(storage_scripts, btc_scripts, "Output scripts in storage do not match those in the original block");
    }
    drop(storage_owner);
    info!("Validation successful");
    Ok(())
}

