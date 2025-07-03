use crate::*;
use redb::Database;
use redbit::AppError;
use std::sync::Arc;

pub async fn run(db: Arc<Database>) -> Result<(), AppError> {
    let blocks = Block::sample_many(2);

    println!("Persisting blocks:");
    let write_tx = db.begin_write()?;
    Block::store_many(&write_tx, &blocks)?;
    write_tx.commit()?;

    let read_tx = db.begin_read()?;

    println!("Querying blocks:");
    let first_block = Block::first(&read_tx)?.unwrap();
    let last_block = Block::last(&read_tx)?.unwrap();

    Block::take(&read_tx, 100)?;
    Block::get(&read_tx, &first_block.id)?;
    Block::range(&read_tx, &first_block.id, &last_block.id)?;
    Block::get_transactions(&read_tx, &first_block.id)?;
    Block::get_header(&read_tx, &first_block.id)?;
    Block::exists(&read_tx, &first_block.id)?;
    Block::first(&read_tx)?;
    Block::last(&read_tx)?;
    Block::stream_range(db.begin_read()?, first_block.id, last_block.id)?.try_collect::<Vec<Block>>().await?;

    println!("Querying block headers:");
    let first_block_header = BlockHeader::first(&read_tx)?.unwrap();
    let last_block_header = BlockHeader::last(&read_tx)?.unwrap();

    BlockHeader::get_by_hash(&read_tx, &first_block_header.hash)?;
    BlockHeader::get_by_timestamp(&read_tx, &first_block_header.timestamp)?;
    BlockHeader::get_by_merkle_root(&read_tx, &first_block_header.merkle_root)?;

    BlockHeader::take(&read_tx, 100)?;
    BlockHeader::get(&read_tx, &first_block_header.id)?;
    BlockHeader::range(&read_tx, &first_block_header.id, &last_block_header.id)?;
    BlockHeader::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
    BlockHeader::stream_by_hash(db.begin_read()?, first_block_header.hash)?.try_collect::<Vec<BlockHeader>>().await?;
    BlockHeader::stream_by_timestamp(db.begin_read()?, first_block_header.timestamp)?.try_collect::<Vec<BlockHeader>>().await?;
    BlockHeader::stream_by_merkle_root(db.begin_read()?, first_block_header.merkle_root)?.try_collect::<Vec<BlockHeader>>().await?;
    BlockHeader::stream_range(db.begin_read()?, first_block_header.id, last_block_header.id)?.try_collect::<Vec<BlockHeader>>().await?;
    BlockHeader::stream_range_by_timestamp(db.begin_read()?, first_block_header.timestamp, last_block_header.timestamp)?.try_collect::<Vec<BlockHeader>>().await?;

    println!("Querying transactions:");
    let first_transaction = Transaction::first(&read_tx)?.unwrap();
    let last_transaction = Transaction::last(&read_tx)?.unwrap();

    Transaction::get_by_hash(&read_tx, &first_transaction.hash)?;
    Transaction::take(&read_tx, 100)?;
    Transaction::get(&read_tx, &first_transaction.id)?;
    Transaction::range(&read_tx, &first_transaction.id, &last_transaction.id)?;
    Transaction::get_utxos(&read_tx, &first_transaction.id)?;
    Transaction::get_inputs(&read_tx, &first_transaction.id)?;
    Transaction::parent_key(&read_tx, &first_transaction.id)?;
    Transaction::stream_ids_by_hash(&read_tx, &first_transaction.hash)?.try_collect::<Vec<TxPointer>>().await?;
    Transaction::stream_by_hash(db.begin_read()?, first_transaction.hash)?.try_collect::<Vec<Transaction>>().await?;
    Transaction::stream_range(db.begin_read()?, first_transaction.id, last_transaction.id)?.try_collect::<Vec<Transaction>>().await?;

    println!("Querying utxos:");
    let first_utxo = Utxo::first(&read_tx)?.unwrap();
    let last_utxo = Utxo::last(&read_tx)?.unwrap();

    Utxo::get_by_address(&read_tx, &first_utxo.address)?;
    Utxo::get_by_datum(&read_tx, &first_utxo.datum)?;

    Utxo::take(&read_tx, 100)?;
    Utxo::get(&read_tx, &first_utxo.id)?;
    Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id)?;
    Utxo::get_assets(&read_tx, &first_utxo.id)?;
    Utxo::parent_key(&read_tx, &first_utxo.id)?;
    Utxo::get_tree(&read_tx, &first_utxo.id)?;
    let first_input_ref = InputRef::first(&read_tx)?.unwrap();
    let last_input_ref = InputRef::last(&read_tx)?.unwrap();
    Utxo::stream_ids_by_address(&read_tx, &first_utxo.address)?.try_collect::<Vec<UtxoPointer>>().await?;
    Utxo::stream_range(db.begin_read()?, first_utxo.id, last_utxo.id)?.try_collect::<Vec<Utxo>>().await?;

    Utxo::stream_by_address(db.begin_read()?, first_utxo.address)?.try_collect::<Vec<Utxo>>().await?;
    Utxo::stream_by_datum(db.begin_read()?, first_utxo.datum)?.try_collect::<Vec<Utxo>>().await?;

    InputRef::take(&read_tx, 100)?;
    InputRef::exists(&read_tx, &first_input_ref.id)?;
    InputRef::get(&read_tx, &first_input_ref.id)?;
    InputRef::range(&read_tx, &first_input_ref.id, &last_input_ref.id)?;
    InputRef::parent_key(&read_tx, &first_input_ref.id)?;
    InputRef::stream_range(db.begin_read()?, first_input_ref.id, last_input_ref.id)?.try_collect::<Vec<InputRef>>().await?;

    println!("Querying assets:");
    let first_asset = Asset::first(&read_tx)?.unwrap();
    let last_asset = Asset::last(&read_tx)?.unwrap();

    Asset::get_by_name(&read_tx, &first_asset.name)?;
    Asset::get_by_policy_id(&read_tx, &first_asset.policy_id)?;

    Asset::take(&read_tx, 100)?;
    Asset::get(&read_tx, &first_asset.id)?;
    Asset::range(&read_tx, &first_asset.id, &last_asset.id)?;
    Asset::parent_key(&read_tx, &first_asset.id)?;
    Asset::stream_ids_by_policy_id(&read_tx, &first_asset.policy_id)?.try_collect::<Vec<AssetPointer>>().await?;
    Asset::stream_by_policy_id(db.begin_read()?, first_asset.policy_id)?.try_collect::<Vec<Asset>>().await?;
    Asset::stream_by_name(db.begin_read()?, first_asset.name)?.try_collect::<Vec<Asset>>().await?;
    Asset::stream_range(db.begin_read()?, first_asset.id, last_asset.id)?.try_collect::<Vec<Asset>>().await?;

    println!("Deleting blocks:");
    for block in blocks.iter() {
        Block::delete_and_commit(&db, &block.id)?
    }
    Ok(())
}
