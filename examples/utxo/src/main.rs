use std::fs::File;
use pprof::ProfilerGuard;
use utxo::*;

fn demo() -> Result<(), DbEngineError> {
    let db = redb::Database::create(std::env::temp_dir().join("my_db.redb"))?;
    let blocks = get_blocks(10, 5, 5, 5);

    println!("Persisting blocks:");
    for block in blocks.iter() {
        Block::store_and_commit(&db, block)?
    }

    let read_tx = db.begin_read()?;

    println!("Querying blocks:");
    let first_block = Block::first(&read_tx)?.unwrap();
    let last_block = Block::last(&read_tx)?.unwrap();

    Block::all(&read_tx)?;
    Block::get(&read_tx, &first_block.id)?;
    Block::range(&read_tx, &first_block.id, &last_block.id)?;
    Block::get_transactions(&read_tx, &first_block.id)?;
    Block::get_header(&read_tx, &first_block.id)?;

    println!("Querying block headers:");
    let first_block_header = BlockHeader::first(&read_tx)?.unwrap();
    let last_block_header = BlockHeader::last(&read_tx)?.unwrap();

    BlockHeader::all(&read_tx)?;
    BlockHeader::get(&read_tx, &first_block_header.id)?;
    BlockHeader::range(&read_tx, &first_block_header.id, &last_block_header.id)?;
    BlockHeader::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
    BlockHeader::get_by_hash(&read_tx, &first_block_header.hash)?;
    BlockHeader::get_by_timestamp(&read_tx, &first_block_header.timestamp)?;
    BlockHeader::get_by_merkle_root(&read_tx, &first_block_header.merkle_root)?;

    println!("Querying transactions:");
    let first_transaction = Transaction::first(&read_tx)?.unwrap();
    let last_transaction = Transaction::last(&read_tx)?.unwrap();

    Transaction::all(&read_tx)?;
    Transaction::get(&read_tx, &first_transaction.id)?;
    Transaction::get_by_hash(&read_tx, &first_transaction.hash)?;
    Transaction::range(&read_tx, &first_transaction.id, &last_transaction.id)?;
    Transaction::get_utxos(&read_tx, &first_transaction.id)?;

    println!("Querying utxos:");
    let first_utxo = Utxo::first(&read_tx)?.unwrap();
    let last_utxo = Utxo::last(&read_tx)?.unwrap();

    Utxo::all(&read_tx)?;
    Utxo::get(&read_tx, &first_utxo.id)?;
    Utxo::get_by_address(&read_tx, &first_utxo.address)?;
    Utxo::get_by_datum(&read_tx, &first_utxo.datum)?;
    Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id)?;
    Utxo::get_assets(&read_tx, &first_utxo.id)?;

    println!("Querying assets:");
    let first_asset = Asset::first(&read_tx)?.unwrap();
    let last_asset = Asset::last(&read_tx)?.unwrap();

    Asset::all(&read_tx)?;
    Asset::get(&read_tx, &first_asset.id)?;
    Asset::get_by_name(&read_tx, &first_asset.name)?;
    Asset::get_by_policy_id(&read_tx, &first_asset.policy_id)?;
    Asset::range(&read_tx, &first_asset.id, &last_asset.id)?;

    println!("Deleting blocks:");
    for block in blocks.iter() {
        Block::delete_and_commit(&db, &block.id)?
    }
    Ok(())
}

fn main() {
    let guard = ProfilerGuard::new(100).unwrap();
    demo().unwrap();
    if let Ok(report) = guard.report().build() {
        let mut file = File::create(std::env::temp_dir().join("flamegraph.svg")).unwrap();
        report.flamegraph(&mut file).unwrap();
        println!("Flamegraph written to flamegraph.svg");
    }

}
