use anyhow::Result;
use redbit::*;
use std::sync::Arc;
use demo::model_v1::*;

#[tokio::main]
async fn main() -> Result<()> {
    let storage = Storage::temp("showcase", 1, true)?;
    let blocks = Block::sample_many(2);
    let block_heights: Vec<Height> = blocks.iter().map(|b|b.height).collect();
    println!("Persisting blocks:");
    let write_tx = storage.begin_write()?;
    Block::store_many(&write_tx, blocks)?;
    write_tx.commit()?;

    let read_tx = storage.begin_read()?;

    let first_block = Block::first(&read_tx)?.unwrap();
    let last_block = Block::last(&read_tx)?.unwrap();

    Block::take(&read_tx, 100)?;
    Block::get(&read_tx, &first_block.height)?;
    Block::range(&read_tx, &first_block.height, &last_block.height, None)?;
    Block::get_transactions(&read_tx, &first_block.height)?;
    Block::get_header(&read_tx, &first_block.height)?;
    Block::exists(&read_tx, &first_block.height)?;
    Block::first(&read_tx)?;
    Block::last(&read_tx)?;
    Block::stream_range(storage.begin_read()?, first_block.height, last_block.height, None)?.try_collect::<Vec<Block>>().await?;

    let block_infos = Block::table_info(Arc::clone(&storage))?;
    println!("Block persisted with tables :");
    for info in block_infos {
        println!("{}", serde_json::to_string_pretty(&info)?);
    }

    let first_block_header = Header::first(&read_tx)?.unwrap();
    let last_block_header = Header::last(&read_tx)?.unwrap();

    Header::get_by_hash(&read_tx, &first_block_header.hash)?;
    Header::get_by_timestamp(&read_tx, &first_block_header.timestamp)?;
    Header::take(&read_tx, 100)?;
    Header::get(&read_tx, &first_block_header.height)?;
    Header::range(&read_tx, &first_block_header.height, &last_block_header.height, None)?;
    Header::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
    Header::stream_by_hash(storage.begin_read()?, first_block_header.hash, None)?.try_collect::<Vec<Header>>().await?;
    Header::stream_by_timestamp(storage.begin_read()?, first_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
    Header::stream_range(storage.begin_read()?, first_block_header.height, last_block_header.height, None)?.try_collect::<Vec<Header>>().await?;
    Header::stream_range_by_timestamp(storage.begin_read()?, first_block_header.timestamp, last_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;

    let block_header_infos = Header::table_info(Arc::clone(&storage))?;
    println!("\nBlock header persisted with tables :");
    for info in block_header_infos {
        println!("{}", serde_json::to_string_pretty(&info)?);
    }

    let first_transaction = Transaction::first(&read_tx)?.unwrap();
    let last_transaction = Transaction::last(&read_tx)?.unwrap();

    Transaction::get_ids_by_hash(&read_tx, &first_transaction.hash)?;
    Transaction::get_by_hash(&read_tx, &first_transaction.hash)?;
    Transaction::take(&read_tx, 100)?;
    Transaction::get(&read_tx, &first_transaction.id)?;
    Transaction::range(&read_tx, &first_transaction.id, &last_transaction.id, None)?;
    Transaction::get_utxos(&read_tx, &first_transaction.id)?;
    Transaction::get_maybe_value(&read_tx, &first_transaction.id)?;
    Transaction::parent_key(&read_tx, &first_transaction.id)?;
    Transaction::stream_ids_by_hash(storage.begin_read()?, first_transaction.hash.clone())?.try_collect::<Vec<BlockPointer>>().await?;
    Transaction::stream_by_hash(storage.begin_read()?, first_transaction.hash.clone(), None)?.try_collect::<Vec<Transaction>>().await?;
    Transaction::stream_range(storage.begin_read()?, first_transaction.id, last_transaction.id, None)?.try_collect::<Vec<Transaction>>().await?;

    let transaction_infos = Transaction::table_info(Arc::clone(&storage))?;
    println!("\nTransaction persisted with tables :");
    for info in transaction_infos {
        println!("{}", serde_json::to_string_pretty(&info)?);
    }

    let first_utxo = Utxo::first(&read_tx)?.unwrap();
    let last_utxo = Utxo::last(&read_tx)?.unwrap();

    Utxo::get_by_address(&read_tx, &first_utxo.address)?;
    Utxo::get_ids_by_address(&read_tx, &first_utxo.address)?;
    Utxo::take(&read_tx, 100)?;
    Utxo::get(&read_tx, &first_utxo.id)?;
    Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id, None)?;
    Utxo::get_assets(&read_tx, &first_utxo.id)?;
    Utxo::parent_key(&read_tx, &first_utxo.id)?;
    Utxo::stream_ids_by_address(storage.begin_read()?, first_utxo.address.clone())?.try_collect::<Vec<TransactionPointer>>().await?;
    Utxo::stream_range(storage.begin_read()?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
    Utxo::stream_by_address(storage.begin_read()?, first_utxo.address.clone(), None)?.try_collect::<Vec<Utxo>>().await?;
    // even streaming parents is possible
    Utxo::stream_transactions_by_address(storage.begin_read()?, first_utxo.address, None)?.try_collect::<Vec<Transaction>>().await?;

    let utxo_infos = Utxo::table_info(Arc::clone(&storage))?;
    println!("\nUtxo persisted with tables :");
    for info in utxo_infos {
        println!("{}", serde_json::to_string_pretty(&info)?);
    }

    let first_asset = Asset::first(&read_tx)?.unwrap();
    let last_asset = Asset::last(&read_tx)?.unwrap();

    Asset::get_by_name(&read_tx, &first_asset.name)?;
    Asset::take(&read_tx, 100)?;
    Asset::get(&read_tx, &first_asset.id)?;
    Asset::range(&read_tx, &first_asset.id, &last_asset.id, None)?;
    Asset::parent_key(&read_tx, &first_asset.id)?;
    Asset::stream_by_name(storage.begin_read()?, first_asset.name.clone(), None)?.try_collect::<Vec<Asset>>().await?;
    Asset::stream_range(storage.begin_read()?, first_asset.id, last_asset.id, None)?.try_collect::<Vec<Asset>>().await?;
    // even streaming parents is possible
    Asset::stream_utxos_by_name(storage.begin_read()?, first_asset.name, None)?.try_collect::<Vec<Utxo>>().await?;

    let asset_infos = Asset::table_info(Arc::clone(&storage))?;
    println!("\nAsset persisted with tables :");
    for info in asset_infos {
        println!("{}", serde_json::to_string_pretty(&info)?);
    }


    println!("\nDeleting blocks:");
    for height in block_heights.iter() {
        Block::delete_and_commit(Arc::clone(&storage), height)?;
    }
    Ok(())
}
