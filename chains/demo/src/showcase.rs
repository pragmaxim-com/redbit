use anyhow::Result;
use redbit::*;
use std::sync::Arc;
use demo::model_v1::*;
use redbit::storage::init::StorageOwner;

#[tokio::main]
async fn main() -> Result<()> {
    let (storage_owner, storage) = StorageOwner::temp("showcase", 1, true).await?;
    let blocks = Block::sample_many(100);
    let block_heights: Vec<Height> = blocks.iter().map(|b|b.height).collect();
    println!("Persisting blocks:");
    let ctx = Block::begin_write_ctx(&storage)?;
    Block::store_many(&ctx, blocks, true)?;
    let _ = ctx.two_phase_commit_and_close(MutationType::Writes)?;

    let block_read_ctx = Block::begin_read_ctx(&storage)?;
    
    let first_block = Block::first(&block_read_ctx)?.unwrap();
    let last_block = Block::last(&block_read_ctx)?.unwrap();

    Block::take(&block_read_ctx, 100)?;
    Block::get(&block_read_ctx, first_block.height)?;
    Block::range(&block_read_ctx, first_block.height, last_block.height, None)?;
    Block::exists(&block_read_ctx, first_block.height)?;
    Block::first(&block_read_ctx)?;
    Block::last(&block_read_ctx)?;

    let tx_read_ctx = &block_read_ctx.transactions;
    let header_read_ctx = &block_read_ctx.header;
    Block::get_transactions(tx_read_ctx, first_block.height)?;
    Block::get_header(header_read_ctx, first_block.height)?;

    Block::table_info(&storage)?;

    let first_block_header = Header::first(header_read_ctx)?.unwrap();
    let last_block_header = Header::last(header_read_ctx)?.unwrap();

    Header::get_by_hash(header_read_ctx, &first_block_header.hash)?;
    Header::get_by_timestamp(header_read_ctx, &first_block_header.timestamp)?;
    Header::take(header_read_ctx, 100)?;
    Header::get(header_read_ctx, first_block_header.height)?;
    Header::range(header_read_ctx, first_block_header.height, last_block_header.height, None)?;
    Header::range_by_timestamp(header_read_ctx, &first_block_header.timestamp, &last_block_header.timestamp)?;

    let first_transaction = Transaction::first(tx_read_ctx)?.unwrap();
    let last_transaction = Transaction::last(tx_read_ctx)?.unwrap();

    Transaction::get_ids_by_hash(tx_read_ctx, &first_transaction.hash)?;
    Transaction::get_by_hash(tx_read_ctx, &first_transaction.hash)?;
    Transaction::take(tx_read_ctx, 100)?;
    Transaction::get(tx_read_ctx, first_transaction.id)?;
    Transaction::range(tx_read_ctx, first_transaction.id, last_transaction.id, None)?;
    Transaction::parent_key(first_transaction.id)?;

    let utxo_read_ctx = &tx_read_ctx.utxos;
    let maybe_value_read_ctx = &tx_read_ctx.maybe;

    Transaction::get_utxos(utxo_read_ctx, first_transaction.id)?;
    Transaction::get_maybe(maybe_value_read_ctx, first_transaction.id)?;

    let first_utxo = Utxo::first(utxo_read_ctx)?.unwrap();
    let last_utxo = Utxo::last(utxo_read_ctx)?.unwrap();

    Utxo::get_by_address(utxo_read_ctx, &first_utxo.address)?;
    Utxo::get_ids_by_address(utxo_read_ctx, &first_utxo.address)?;
    Utxo::take(utxo_read_ctx, 100)?;
    Utxo::get(utxo_read_ctx, first_utxo.id)?;
    Utxo::range(utxo_read_ctx, first_utxo.id, last_utxo.id, None)?;
    Utxo::parent_key(first_utxo.id)?;

    let asset_read_ctx = &utxo_read_ctx.assets;
    Utxo::get_assets(asset_read_ctx, first_utxo.id)?;

    let first_asset = Asset::first(asset_read_ctx)?.unwrap();
    let last_asset = Asset::last(asset_read_ctx)?.unwrap();

    Asset::get_by_name(asset_read_ctx, &first_asset.name)?;
    Asset::take(asset_read_ctx, 100)?;
    Asset::get(asset_read_ctx, first_asset.id)?;
    Asset::range(asset_read_ctx, first_asset.id, last_asset.id, None)?;
    Asset::parent_key(first_asset.id)?;

    /* Streaming examples */
    Block::stream_range(Block::begin_read_ctx(&storage)?, first_block.height, last_block.height, None)?.try_collect::<Vec<Block>>().await?;
    Header::stream_by_hash(Header::begin_read_ctx(&storage)?, first_block_header.hash, None)?.try_collect::<Vec<Header>>().await?;
    Header::stream_by_timestamp(Header::begin_read_ctx(&storage)?, first_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
    Header::stream_range(Header::begin_read_ctx(&storage)?, first_block_header.height, last_block_header.height, None)?.try_collect::<Vec<Header>>().await?;
    Header::stream_range_by_timestamp(Header::begin_read_ctx(&storage)?, first_block_header.timestamp, last_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
    Transaction::stream_ids_by_hash(Transaction::begin_read_ctx(&storage)?, first_transaction.hash)?.try_collect::<Vec<BlockPointer>>().await?;
    Transaction::stream_by_hash(Transaction::begin_read_ctx(&storage)?, first_transaction.hash, None)?.try_collect::<Vec<Transaction>>().await?;
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
    drop(storage_owner);
    Ok(())
}
