use utxo::*;

fn main() {
    let db = redb::Database::create(std::env::temp_dir().join("my_db.redb")).unwrap();
    let blocks = get_blocks(5, 5, 5, 5);

    blocks.iter().for_each(|block| Block::store_and_commit(&db, &block).unwrap());

    let read_tx = db.begin_read().unwrap();

    let first_block = Block::first(&read_tx).unwrap().unwrap();
    let last_block = Block::last(&read_tx).unwrap().unwrap();

    Block::all(&read_tx).unwrap();
    Block::get(&read_tx, &first_block.id).unwrap();
    Block::range(&read_tx, &first_block.id, &last_block.id).unwrap();
    Block::get_transactions(&read_tx, &first_block.id).unwrap();
    Block::get_header(&read_tx, &first_block.id).unwrap();

    let first_block_header = BlockHeader::first(&read_tx).unwrap().unwrap();
    let last_block_header = BlockHeader::last(&read_tx).unwrap().unwrap();

    BlockHeader::all(&read_tx).unwrap();
    BlockHeader::get(&read_tx, &first_block_header.id).unwrap();
    BlockHeader::range(&read_tx, &first_block_header.id, &last_block_header.id).unwrap();
    BlockHeader::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp).unwrap();
    BlockHeader::get_by_hash(&read_tx, &first_block_header.hash).unwrap();
    BlockHeader::get_by_timestamp(&read_tx, &first_block_header.timestamp).unwrap();
    BlockHeader::get_by_merkle_root(&read_tx, &first_block_header.merkle_root).unwrap();

    let first_transaction = Transaction::first(&read_tx).unwrap().unwrap();
    let last_transaction = Transaction::last(&read_tx).unwrap().unwrap();

    Transaction::all(&read_tx).unwrap();
    Transaction::get(&read_tx, &first_transaction.id).unwrap();
    Transaction::get_by_hash(&read_tx, &first_transaction.hash).unwrap();
    Transaction::range(&read_tx, &first_transaction.id, &last_transaction.id).unwrap();
    Transaction::get_utxos(&read_tx, &first_transaction.id).unwrap();

    let first_utxo = Utxo::first(&read_tx).unwrap().unwrap();
    let last_utxo = Utxo::last(&read_tx).unwrap().unwrap();

    Utxo::all(&read_tx).unwrap();
    Utxo::get(&read_tx, &first_utxo.id).unwrap();
    Utxo::get_by_address(&read_tx, &first_utxo.address).unwrap();
    Utxo::get_by_datum(&read_tx, &first_utxo.datum).unwrap();
    Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id).unwrap();
    Utxo::get_assets(&read_tx, &first_utxo.id).unwrap();

    let first_asset = Asset::first(&read_tx).unwrap().unwrap();
    let last_asset = Asset::last(&read_tx).unwrap().unwrap();

    Asset::all(&read_tx).unwrap();
    Asset::get(&read_tx, &first_asset.id).unwrap();
    Asset::get_by_name(&read_tx, &first_asset.name).unwrap();
    Asset::get_by_policy_id(&read_tx, &first_asset.policy_id).unwrap();
    Asset::range(&read_tx, &first_asset.id, &last_asset.id).unwrap();

    blocks.iter().for_each(|block| Block::delete_and_commit(&db, &block.id).unwrap());
}
