use utxo::*;

fn create_test_db() -> (Vec<Block>, redb::Database) {
    let random_number = rand::random::<u32>();
    let db = redb::Database::create(std::env::temp_dir().join(format!("test_db_{}.redb", random_number))).unwrap();
    let blocks = get_blocks(3, 3, 3, 3);
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
    assert_eq!(found_by_hash.len(), 3);
    assert!(found_by_hash.iter().any(|tx| tx.id == transaction.id));
    assert!(found_by_hash.iter().any(|tx| tx.id == transaction.id));
}

#[test]
fn it_should_get_entities_by_index_with_dict() {
    let (blocks, db) = create_test_db();

    let read_tx = db.begin_read().unwrap();
    let utxo = blocks.first().unwrap().transactions.first().unwrap().utxos.first().unwrap();

    let found_by_address = Utxo::get_by_address(&read_tx, &utxo.address).expect("Failed to query by address");
    assert_eq!(found_by_address.len(), 27);
    assert!(found_by_address.iter().any(|tx| tx.id == utxo.id));
    assert!(found_by_address.iter().any(|tx| tx.id == utxo.id));
}

#[test]
fn it_should_get_entities_by_range_on_index() {
    let (blocks, db) = create_test_db();

    let read_tx = db.begin_read().unwrap();

    let from_timestamp = blocks[1].header.timestamp;
    let to_timestamp = blocks[2].header.timestamp;

    let found_by_timestamp_range = BlockHeader::range_by_timestamp(&read_tx, &from_timestamp, &to_timestamp).expect("Failed to range by timestamp");
    let expected_blocks: Vec<BlockHeader> =
        blocks.into_iter().map(|b| b.header).take_while(|b| b.timestamp >= from_timestamp && b.timestamp <= to_timestamp).collect();
    assert_eq!(found_by_timestamp_range.len(), 3);
    assert_eq!(expected_blocks, found_by_timestamp_range);
}

#[test]
fn it_should_get_entities_by_range_on_pk() {
    let (blocks, db) = create_test_db();

    let read_tx = db.begin_read().unwrap();

    let all_expected_transactions: Vec<Transaction> = blocks.clone().into_iter().flat_map(|b| b.transactions).collect();
    let all_expected_utxos: Vec<Utxo> = blocks.clone().into_iter().flat_map(|b| b.transactions).flat_map(|t| t.utxos).collect();

    let expected_transactions: Vec<Transaction> =
        all_expected_transactions.clone().into_iter().skip(1).take(all_expected_transactions.len() - 2).collect();
    let expected_utxos: Vec<Utxo> = all_expected_utxos.clone().into_iter().skip(1).take(all_expected_utxos.len() - 2).collect();

    let found_tx_by_pk_range = Transaction::range(&read_tx, &expected_transactions.first().unwrap().id, &expected_transactions.last().unwrap().id)
        .expect("Failed to range by pk");
    let found_utxo_by_pk_range =
        Utxo::range(&read_tx, &expected_utxos.first().unwrap().id, &expected_utxos.last().unwrap().id).expect("Failed to range by pk");

    let all_transactions =
        Transaction::range(&read_tx, &all_expected_transactions.first().unwrap().id, &all_expected_transactions.last().unwrap().id)
            .expect("Failed to range by pk");
    let all_utxos =
        Utxo::range(&read_tx, &all_expected_utxos.first().unwrap().id, &all_expected_utxos.last().unwrap().id).expect("Failed to range by pk");

    assert_eq!(expected_utxos, found_utxo_by_pk_range);
    assert_eq!(all_expected_utxos, all_utxos);
    assert_eq!(all_expected_transactions, all_transactions);
    assert_eq!(expected_transactions, found_tx_by_pk_range);
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
