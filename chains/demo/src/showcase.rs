use anyhow::Result;
use redbit::*;
use std::sync::Arc;
use demo::model_v1::*;

#[tokio::main]
async fn main() -> Result<()> {
    let storage = Storage::temp("showcase", 1, true).await?;
    let blocks = Block::sample_many(2);
    let block_heights: Vec<Height> = blocks.iter().map(|b|b.height).collect();
    println!("Persisting blocks:");
    for block in blocks {
        Block::persist(Arc::clone(&storage), block)?;
    }

    let block_tx = Block::begin_read_ctx(&storage)?;
    let transaction_tx = &block_tx.transactions;
    let header_tx = &block_tx.header;
    let utxo_tx = &transaction_tx.utxos;
    let maybe_value_tx = &transaction_tx.maybe;
    let asset_tx = &utxo_tx.assets;

    let first_block = Block::first(&block_tx)?.unwrap();
    let last_block = Block::last(&block_tx)?.unwrap();

    Block::take(&block_tx, 100)?;
    Block::get(&block_tx, &first_block.height)?;
    Block::range(&block_tx, &first_block.height, &last_block.height, None)?;
    Block::get_transactions(&transaction_tx, &first_block.height)?;
    Block::get_header(&header_tx, &first_block.height)?;
    Block::exists(&block_tx, &first_block.height)?;
    Block::first(&block_tx)?;
    Block::last(&block_tx)?;

    let block_infos = Block::table_info(&storage)?;
    println!("Block persisted with tables :");
    for info in block_infos {
        println!("{}", serde_json::to_string_pretty(&info)?);
    }

    let first_block_header = Header::first(&header_tx)?.unwrap();
    let last_block_header = Header::last(&header_tx)?.unwrap();

    Header::get_by_hash(&header_tx, &first_block_header.hash)?;
    Header::get_by_timestamp(&header_tx, &first_block_header.timestamp)?;
    Header::take(&header_tx, 100)?;
    Header::get(&header_tx, &first_block_header.height)?;
    Header::range(&header_tx, &first_block_header.height, &last_block_header.height, None)?;
    Header::range_by_timestamp(&header_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;

    let block_header_infos = Header::table_info(&storage)?;
    println!("\nBlock header persisted with tables :");
    for info in block_header_infos {
        println!("{}", serde_json::to_string_pretty(&info)?);
    }

    let first_transaction = Transaction::first(&transaction_tx)?.unwrap();
    let last_transaction = Transaction::last(&transaction_tx)?.unwrap();

    Transaction::get_ids_by_hash(&transaction_tx, &first_transaction.hash)?;
    Transaction::get_by_hash(&transaction_tx, &first_transaction.hash)?;
    Transaction::take(&transaction_tx, 100)?;
    Transaction::get(&transaction_tx, &first_transaction.id)?;
    Transaction::range(&transaction_tx, &first_transaction.id, &last_transaction.id, None)?;
    Transaction::get_utxos(&utxo_tx, &first_transaction.id)?;
    Transaction::get_maybe(&maybe_value_tx, &first_transaction.id)?;
    Transaction::parent_key(&first_transaction.id)?;

    let transaction_infos = Transaction::table_info(&storage)?;
    println!("\nTransaction persisted with tables :");
    for info in transaction_infos {
        println!("{}", serde_json::to_string_pretty(&info)?);
    }

    let first_utxo = Utxo::first(&utxo_tx)?.unwrap();
    let last_utxo = Utxo::last(&utxo_tx)?.unwrap();

    Utxo::get_by_address(&utxo_tx, &first_utxo.address)?;
    Utxo::get_ids_by_address(&utxo_tx, &first_utxo.address)?;
    Utxo::take(&utxo_tx, 100)?;
    Utxo::get(&utxo_tx, &first_utxo.id)?;
    Utxo::range(&utxo_tx, &first_utxo.id, &last_utxo.id, None)?;
    Utxo::get_assets(&asset_tx, &first_utxo.id)?;
    Utxo::parent_key(&first_utxo.id)?;

    let utxo_infos = Utxo::table_info(&storage)?;
    println!("\nUtxo persisted with tables :");
    for info in utxo_infos {
        println!("{}", serde_json::to_string_pretty(&info)?);
    }

    let first_asset = Asset::first(&asset_tx)?.unwrap();
    let last_asset = Asset::last(&asset_tx)?.unwrap();

    Asset::get_by_name(&asset_tx, &first_asset.name)?;
    Asset::take(&asset_tx, 100)?;
    Asset::get(&asset_tx, &first_asset.id)?;
    Asset::range(&asset_tx, &first_asset.id, &last_asset.id, None)?;
    Asset::parent_key(&first_asset.id)?;

    let asset_infos = Asset::table_info(&storage)?;
    println!("\nAsset persisted with tables :");
    for info in asset_infos {
        println!("{}", serde_json::to_string_pretty(&info)?);
    }

    /* Streaming examples */
    Block::stream_range(Block::begin_read_ctx(&storage)?, first_block.height, last_block.height, None)?.try_collect::<Vec<Block>>().await?;
    Header::stream_by_hash(Header::begin_read_ctx(&storage)?, first_block_header.hash, None)?.try_collect::<Vec<Header>>().await?;
    Header::stream_by_timestamp(Header::begin_read_ctx(&storage)?, first_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
    Header::stream_range(Header::begin_read_ctx(&storage)?, first_block_header.height, last_block_header.height, None)?.try_collect::<Vec<Header>>().await?;
    Header::stream_range_by_timestamp(Header::begin_read_ctx(&storage)?, first_block_header.timestamp, last_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
    Transaction::stream_ids_by_hash(Transaction::begin_read_ctx(&storage)?, first_transaction.hash.clone())?.try_collect::<Vec<BlockPointer>>().await?;
    Transaction::stream_by_hash(Transaction::begin_read_ctx(&storage)?, first_transaction.hash.clone(), None)?.try_collect::<Vec<Transaction>>().await?;
    Transaction::stream_range(Transaction::begin_read_ctx(&storage)?, first_transaction.id, last_transaction.id, None)?.try_collect::<Vec<Transaction>>().await?;
    Utxo::stream_ids_by_address(Utxo::begin_read_ctx(&storage)?, first_utxo.address.clone())?.try_collect::<Vec<TransactionPointer>>().await?;
    Utxo::stream_range(Utxo::begin_read_ctx(&storage)?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
    Utxo::stream_by_address(Utxo::begin_read_ctx(&storage)?, first_utxo.address.clone(), None)?.try_collect::<Vec<Utxo>>().await?;
    // streaming parents
    Utxo::stream_transactions_by_address(Transaction::begin_read_ctx(&storage)?, first_utxo.address, None)?.try_collect::<Vec<Transaction>>().await?;
    Asset::stream_by_name(Asset::begin_read_ctx(&storage)?, first_asset.name.clone(), None)?.try_collect::<Vec<Asset>>().await?;
    Asset::stream_range(Asset::begin_read_ctx(&storage)?, first_asset.id, last_asset.id, None)?.try_collect::<Vec<Asset>>().await?;
    // streaming parents
    Asset::stream_utxos_by_name(Utxo::begin_read_ctx(&storage)?, first_asset.name, None)?.try_collect::<Vec<Utxo>>().await?;

    println!("\nDeleting blocks:");
    for height in block_heights.into_iter() {
        Block::remove(Arc::clone(&storage), height)?;
    }
    Ok(())
}
