use crate::*;
use redb::Database;
use redbit::AppError;
use std::sync::Arc;

pub fn run(db: Arc<Database>) -> Result<(), AppError> {
    let blocks = Block::sample_many(2);

    println!("Persisting blocks:");
    let write_tx = db.begin_write()?;
    Block::store_many(&write_tx, &blocks)?;
    write_tx.commit()?;
    
    let read_tx = db.begin_read()?;

    println!("Querying blocks:");
    let first_block = Block::first(&read_tx)?.unwrap();
    let last_block = Block::last(&read_tx)?.unwrap();

    Block::take(&read_tx, 1000)?;
    Block::get(&read_tx, &first_block.id)?;
    Block::range(&read_tx, &first_block.id, &last_block.id)?;
    Block::get_transactions(&read_tx, &first_block.id)?;
    Block::get_header(&read_tx, &first_block.id)?;
    Block::exists(&read_tx, &first_block.id)?;
    Block::first(&read_tx)?;
    Block::last(&read_tx)?;

    println!("Querying block headers:");
    let first_block_header = BlockHeader::first(&read_tx)?.unwrap();
    let last_block_header = BlockHeader::last(&read_tx)?.unwrap();

    BlockHeader::take(&read_tx, 1000)?;
    BlockHeader::get(&read_tx, &first_block_header.id)?;
    BlockHeader::range(&read_tx, &first_block_header.id, &last_block_header.id)?;
    BlockHeader::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
    BlockHeader::get_by_hash(&read_tx, &first_block_header.hash)?;
    BlockHeader::get_by_timestamp(&read_tx, &first_block_header.timestamp)?;
    BlockHeader::get_by_merkle_root(&read_tx, &first_block_header.merkle_root)?;

    println!("Querying transactions:");
    let first_transaction = Transaction::first(&read_tx)?.unwrap();
    let last_transaction = Transaction::last(&read_tx)?.unwrap();

    Transaction::take(&read_tx, 1000)?;
    Transaction::get(&read_tx, &first_transaction.id)?;
    Transaction::get_by_hash(&read_tx, &first_transaction.hash)?;
    Transaction::range(&read_tx, &first_transaction.id, &last_transaction.id)?;
    Transaction::get_utxos(&read_tx, &first_transaction.id)?;
    Transaction::get_inputs(&read_tx, &first_transaction.id)?;
    Transaction::parent_key(&read_tx, &first_transaction.id)?;
    Transaction::get_ids_by_hash(&read_tx, &first_transaction.hash)?;
    
    println!("Querying utxos:");
    let first_utxo = Utxo::first(&read_tx)?.unwrap();
    let last_utxo = Utxo::last(&read_tx)?.unwrap();

    Utxo::take(&read_tx, 1000)?;
    Utxo::get(&read_tx, &first_utxo.id)?;
    Utxo::get_by_address(&read_tx, &first_utxo.address)?;
    Utxo::get_by_datum(&read_tx, &first_utxo.datum)?;
    Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id)?;
    Utxo::get_assets(&read_tx, &first_utxo.id)?;
    Utxo::parent_key(&read_tx, &first_utxo.id)?;
    Utxo::get_tree(&read_tx, &first_utxo.id)?;
    Utxo::get_ids_by_address(&read_tx, &first_utxo.address)?;

    println!("Querying input refs:");
    let first_input_ref = InputRef::first(&read_tx)?.unwrap();
    let last_input_ref = InputRef::last(&read_tx)?.unwrap();

    InputRef::take(&read_tx, 1000)?;
    InputRef::exists(&read_tx, &first_input_ref.id)?;
    InputRef::get(&read_tx, &first_input_ref.id)?;
    InputRef::range(&read_tx, &first_input_ref.id, &last_input_ref.id)?;
    InputRef::parent_key(&read_tx, &first_input_ref.id)?;


    println!("Querying assets:");
    let first_asset = Asset::first(&read_tx)?.unwrap();
    let last_asset = Asset::last(&read_tx)?.unwrap();

    Asset::take(&read_tx, 1000)?;
    Asset::get(&read_tx, &first_asset.id)?;
    Asset::get_by_name(&read_tx, &first_asset.name)?;
    Asset::get_by_policy_id(&read_tx, &first_asset.policy_id)?;
    Asset::range(&read_tx, &first_asset.id, &last_asset.id)?;
    Asset::parent_key(&read_tx, &first_asset.id)?;
    Asset::get_ids_by_policy_id(&read_tx, &first_asset.policy_id)?;

    println!("Deleting blocks:");
    for block in blocks.iter() {
        Block::delete_and_commit(&db, &block.id)?
    }
    Ok(())
}
