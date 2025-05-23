use std::collections::HashSet;
use utxo::*;

fn create_test_db() -> (Vec<Block>, redb::Database) {
    let random_number = rand::random::<u64>();
    let db = redb::Database::create(std::env::temp_dir().join(format!("test_db_{}.redb", random_number))).unwrap();
    let blocks = get_blocks(4, 4, 4, 4);
    blocks.iter().for_each(|block| Block::store_and_commit(&db, &block).expect("Failed to persist blocks"));
    (blocks, db)
}

#[test]
fn it_should_get_entity_by_unique_id() {
    let (blocks, db) = create_test_db();
    let block = blocks.first().unwrap();
    let found_by_id = Block::get(&db.begin_read().unwrap(), &block.id).expect("Failed to query by ID").unwrap();
    assert_eq!(found_by_id.id, block.id);
    assert_eq!(found_by_id.transactions, block.transactions);
    assert_eq!(found_by_id.header, block.header);
}

#[test]
fn it_should_delete_entity_by_unique_id() {
    let (blocks, db) = create_test_db();
    let block = blocks.first().unwrap();
    let found_by_id = Block::get(&db.begin_read().unwrap(), &block.id).expect("Failed to query by ID").unwrap();
    assert_eq!(found_by_id.id, block.id);

    Block::delete_and_commit(&db, &block.id).expect("Failed to delete by ID");

    let read_tx = db.begin_read().unwrap();

    let block_not_found = Block::get(&read_tx, &block.id).expect("Failed to query by ID after deletion").is_none();
    assert!(block_not_found);

    let transitive_entities_not_found = block.transactions.iter().any(|tx| {
        let tx_not_found = Transaction::get(&read_tx, &tx.id).unwrap().is_none();
        let utxos_not_found = tx.utxos.iter().any(|utxo| {
            let utxo_not_found = Utxo::get(&read_tx, &utxo.id).unwrap().is_none();
            let assets_not_found = utxo.assets.iter().any(|asset| Asset::get(&read_tx, &asset.id).unwrap().is_none());
            utxo_not_found && assets_not_found
        });
        tx_not_found && utxos_not_found
    });
    assert!(transitive_entities_not_found);
}

#[test]
fn it_should_get_entities_by_index() {
    let (blocks, db) = create_test_db();

    let read_tx = db.begin_read().unwrap();
    let transaction = blocks.first().unwrap().transactions.first().unwrap();

    let found_by_hash = Transaction::get_by_hash(&read_tx, &transaction.hash).expect("Failed to query by hash");
    assert_eq!(found_by_hash.len(), 4);
    assert!(found_by_hash.iter().any(|tx| tx.id == transaction.id));
    assert!(found_by_hash.iter().any(|tx| tx.id == transaction.id));
}

#[test]
fn it_should_get_entities_by_index_with_dict() {
    let (blocks, db) = create_test_db();

    let read_tx = db.begin_read().unwrap();
    let utxo = blocks.first().unwrap().transactions.first().unwrap().utxos.first().unwrap();

    let found_by_address = Utxo::get_by_address(&read_tx, &utxo.address).expect("Failed to query by address");
    assert_eq!(found_by_address.len(), 16);
    assert!(found_by_address.iter().any(|tx| tx.id == utxo.id));
    assert!(found_by_address.iter().any(|tx| tx.id == utxo.id));
}

#[test]
fn it_should_get_entities_by_range_on_index() {
    let (blocks, db) = create_test_db();

    let read_tx = db.begin_read().unwrap();

    let from_timestamp = blocks[1].header.timestamp;
    let until_timestamp = blocks[3].header.timestamp;
    let expected_blocks: Vec<BlockHeader> = vec![blocks[1].header.clone(), blocks[2].header.clone()];
    let unique_timestamps: HashSet<Timestamp> = BlockHeader::all(&read_tx).unwrap().iter().map(|h| h.timestamp).collect();
    assert_eq!(unique_timestamps.len(), 4);

    let found_by_timestamp_range = BlockHeader::range_by_timestamp(&read_tx, &from_timestamp, &until_timestamp).expect("Failed to range by timestamp");
    assert_eq!(found_by_timestamp_range.len(), 2);
    assert_eq!(expected_blocks, found_by_timestamp_range);
}

#[test]
fn it_should_get_entities_by_range_on_pk() {
    let (blocks, db) = create_test_db();

    let read_tx = db.begin_read().unwrap();

    let block_pointer_1 = BlockPointer { height: 1 };
    let block_pointer_2 = BlockPointer { height: 2 };
    let block_pointer_3 = BlockPointer { height: 3 };
    let actual_blocks = Block::range(&read_tx, &block_pointer_1, &block_pointer_3).expect("Failed to range by PK");
    let expected_blocks: Vec<Block> = vec![blocks[1].clone(), blocks[2].clone()];

    assert_eq!(expected_blocks.len(), actual_blocks.len());
    assert_eq!(actual_blocks[0].transactions.len(), 4);
    assert_eq!(actual_blocks[1].transactions.len(), 4);
    assert_eq!(expected_blocks, actual_blocks);

    let tx_pointer_1 = TxPointer { block_pointer: block_pointer_1, tx_index: 1 };
    let tx_pointer_2 = TxPointer { block_pointer: block_pointer_2, tx_index: 3 };
    let actual_transactions = Transaction::range(&read_tx, &tx_pointer_1, &tx_pointer_2).expect("Failed to range by PK");
    let mut expected_transactions: Vec<Transaction> = Vec::new();
    expected_transactions.extend(blocks[1].transactions.clone().into_iter().filter(|t| t.id.tx_index >= 1));
    expected_transactions.extend(blocks[2].transactions.clone().into_iter().filter(|t| t.id.tx_index < 3));

    assert_eq!(expected_transactions.len(), actual_transactions.len());
    assert_eq!(expected_transactions, actual_transactions);
    assert!(actual_transactions.iter().all(|t| t.utxos.len() == 4));
}

#[test]
fn it_should_get_related_one_to_many_entities() {
    let (blocks, db) = create_test_db();
    let read_tx = db.begin_read().unwrap();
    let block = blocks.first().unwrap();

    let expected_transactions: Vec<Transaction> = block.transactions.clone();
    let transactions = Block::get_transactions(&read_tx, &block.id).expect("Failed to get transactions");

    let expected_utxos: Vec<Utxo> = expected_transactions.iter().flat_map(|t| t.utxos.clone()).collect();
    let utxos: Vec<Utxo> = transactions.iter().flat_map(|t| t.utxos.clone()).collect();

    let expected_assets: Vec<Asset> = expected_utxos.iter().flat_map(|u| u.assets.clone()).collect();
    let assets: Vec<Asset> = utxos.iter().flat_map(|u| u.assets.clone()).collect();

    assert_eq!(expected_transactions, transactions);
    assert_eq!(expected_utxos, utxos);
    assert_eq!(expected_assets, assets);
}

#[test]
fn it_should_get_related_one_to_one_entity() {
    let (blocks, db) = create_test_db();
    let read_tx = db.begin_read().unwrap();
    let block = blocks.first().unwrap();

    let expected_header: BlockHeader = block.header.clone();
    let header = Block::get_header(&read_tx, &block.id).expect("Failed to get header").unwrap();

    assert_eq!(expected_header, header);
}

#[test]
fn it_should_override_entity() {
    let (blocks, db) = create_test_db();
    let read_tx = db.begin_read().unwrap();
    let block = blocks.first().unwrap();

    let loaded_block = Block::get(&read_tx, &block.id).expect("Failed to get by ID").unwrap();

    assert_eq!(block, &loaded_block);

    Block::store_and_commit(&db, &block).expect("Failed to delete by ID");
    let loaded_block2 = Block::get(&read_tx, &block.id).expect("Failed to get by ID").unwrap();

    assert_eq!(block, &loaded_block2);
}

#[test]
fn it_should_get_first_and_last_entity() {
    let (blocks, db) = create_test_db();

    let read_tx = db.begin_read().unwrap();
    let first_block = Block::first(&read_tx).expect("Failed to get first block").unwrap();
    let last_block = Block::last(&read_tx).expect("Failed to get last block").unwrap();

    assert_eq!(blocks.first().unwrap().id, first_block.id);
    assert_eq!(blocks.last().unwrap().id, last_block.id);
}
