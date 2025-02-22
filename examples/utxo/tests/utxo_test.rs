use utxo::*;

fn create_test_db() -> redb::Database {
    let random_number = rand::random::<u32>();
    redb::Database::create(std::env::temp_dir().join(format!("test_db_{}.redb", random_number))).unwrap()
}

fn create_test_asset(height: Height, tx_index: TxIndex, utxo_index: UtxoIndex, asset_index: AssetIndex) -> Asset {
    Asset {
        id: AssetPointer {
            utxo_pointer: UtxoPointer { tx_pointer: TxPointer { block_pointer: BlockPointer { height }, tx_index }, utxo_index },
            asset_index,
        },
        amount: 100_000,
        name: format!("asset_name_{}", asset_index),
        policy_id: format!("policy_id_{}", asset_index),
    }
}

fn create_test_utxo(block_height: Height, tx_index: TxIndex, utxo_index: UtxoIndex) -> Utxo {
    let assets = (0..2).map(|asset_index| create_test_asset(block_height, tx_index, utxo_index, asset_index)).collect();
    Utxo {
        id: UtxoPointer { tx_pointer: TxPointer { block_pointer: BlockPointer { height: block_height }, tx_index }, utxo_index },
        amount: 999_999,
        datum: format!("datum_{}", block_height),
        address: format!("address_{}", tx_index),
        assets,
    }
}

fn create_test_transaction(block_height: Height, tx_index: TxIndex) -> Transaction {
    let tx_id = TxPointer { block_pointer: BlockPointer { height: block_height }, tx_index };
    let utxos: Vec<Utxo> = (0..2).map(|utxo_index| create_test_utxo(block_height, tx_index, utxo_index)).collect();

    Transaction { id: tx_id, hash: format!("tx_hash_{}", tx_index), utxos }
}

fn create_test_block(hash: Hash, height: Height, timestamp: u64) -> Block {
    let block_id = BlockPointer { height };
    let transactions: Vec<Transaction> = (0..1).map(|tx_index| create_test_transaction(height, tx_index)).collect();

    Block {
        id: block_id.clone(),
        header: BlockHeader { id: block_id.clone(), hash, timestamp, merkle_root: String::from("merkle_root"), nonce: 0 },
        transactions,
    }
}

#[test]
fn it_should_get_entity_by_unique_id() {
    let db = create_test_db();
    let utxo = create_test_utxo(42, 7, 6);

    Utxo::store_and_commit(&db, &utxo).expect("Failed to store utxo");

    let found_by_id = Utxo::get(&db.begin_read().unwrap(), &utxo.id).expect("Failed to query by ID");
    assert_eq!(found_by_id.id, utxo.id);
    assert_eq!(found_by_id.amount, utxo.amount);
    assert_eq!(found_by_id.datum, utxo.datum);
    assert_eq!(found_by_id.address, utxo.address);
}

#[test]
fn it_should_get_entities_by_index() {
    let db = create_test_db();
    let utxo1 = create_test_utxo(42, 7, 6);
    let utxo2 = create_test_utxo(43, 7, 1);

    Utxo::store_and_commit(&db, &utxo1).expect("Failed to store utxo1");
    Utxo::store_and_commit(&db, &utxo2).expect("Failed to store utxo2");

    let read_tx = db.begin_read().unwrap();

    // Test by address (one-to-many)
    let found_by_address = Utxo::get_by_address(&read_tx, &utxo1.address).expect("Failed to query by address");
    assert_eq!(found_by_address.len(), 2);
    assert!(found_by_address.iter().any(|u| u.id == utxo1.id));
    assert!(found_by_address.iter().any(|u| u.id == utxo2.id));

    // Test by datum (one-to-many)
    let found_by_datum1 = Utxo::get_by_datum(&read_tx, &utxo1.datum).expect("Failed to query by datum");
    assert_eq!(found_by_datum1.len(), 1);
    assert_eq!(found_by_datum1[0].id, utxo1.id);

    let found_by_datum2 = Utxo::get_by_datum(&read_tx, &utxo2.datum).expect("Failed to query by datum");
    assert_eq!(found_by_datum2.len(), 1);
    assert_eq!(found_by_datum2[0].id, utxo2.id);
}

#[test]
fn it_should_get_entities_by_range_on_index() {
    let db = create_test_db();
    let mut blocks = Vec::new();
    let write_tx = db.begin_write().unwrap();
    for height in 40..44 {
        let block = create_test_block(format!("unique{}", height), height, u64::from(height));
        blocks.push(block.clone());
        Block::store(&write_tx, &block).expect("Failed to store block");
    }
    write_tx.commit().unwrap();

    let found_by_height_range = BlockHeader::range_by_timestamp(&db.begin_read().unwrap(), &41, &42).expect("Failed to range by height");
    let expected_blocks: Vec<BlockHeader> = blocks.into_iter().map(|b| b.header).filter(|b| b.timestamp == 41 || b.timestamp == 42).collect();
    assert_eq!(found_by_height_range.len(), 2);
    assert_eq!(expected_blocks, found_by_height_range);
}

#[test]
fn it_should_get_entities_by_range_on_pk() {
    let db = create_test_db();
    let mut blocks = Vec::new();
    let write_tx = db.begin_write().unwrap();
    for height in 1..4 {
        let block = create_test_block(format!("unique{}", height), height, u64::from(height));
        blocks.push(block.clone());
        Block::store(&write_tx, &block).expect("Failed to store block");
    }
    write_tx.commit().unwrap();

    let read_tx = db.begin_read().unwrap();

    let all_expected_transactions:Vec<Transaction> = blocks.clone().into_iter().flat_map(|b| b.transactions).collect();
    let all_expected_utxos: Vec<Utxo> = blocks.clone().into_iter().flat_map(|b| b.transactions).flat_map(|t| t.utxos).collect();

    let expected_transactions: Vec<Transaction> = all_expected_transactions.clone().into_iter().skip(1).take(all_expected_transactions.len() - 2).collect();
    let expected_utxos: Vec<Utxo> = all_expected_utxos.clone().into_iter().skip(1).take(all_expected_utxos.len() - 2).collect();

    let found_tx_by_pk_range = Transaction::range(&read_tx, &expected_transactions.first().unwrap().id, &expected_transactions.last().unwrap().id).expect("Failed to range by pk");
    let found_utxo_by_pk_range = Utxo::range(&read_tx, &expected_utxos.first().unwrap().id, &expected_utxos.last().unwrap().id)
        .expect("Failed to range by pk");

    let all_transactions = Transaction::range(&read_tx, &all_expected_transactions.first().unwrap().id, &all_expected_transactions.last().unwrap().id).expect("Failed to range by pk");
    let all_utxos = Utxo::range(&read_tx, &all_expected_utxos.first().unwrap().id, &all_expected_utxos.last().unwrap().id).expect("Failed to range by pk");

    assert_eq!(expected_utxos, found_utxo_by_pk_range);
    assert_eq!(all_expected_utxos, all_utxos);
    assert_eq!(all_expected_transactions, all_transactions);
    assert_eq!(expected_transactions, found_tx_by_pk_range);
}

#[test]
fn it_should_get_related_one_to_many_entities() {
    let db = create_test_db();

    let block = create_test_block(String::from("block_hash_1"), 42, 7);
    Block::store_and_commit(&db, &block).expect("Failed to store block");

    let expected_transactions: Vec<Transaction> = block.transactions;
    let transactions = Block::get_transactions(&db.begin_read().unwrap(), &block.id).expect("Failed to get transactions");

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
    let db = create_test_db();

    let block = create_test_block(String::from("block_hash_1"), 42, 7);
    Block::store_and_commit(&db, &block).expect("Failed to store block");

    let expected_header: BlockHeader = block.header;
    let header = Block::get_header(&db.begin_read().unwrap(), &block.id).expect("Failed to get header");

    assert_eq!(expected_header, header);
}

#[test]
fn it_should_override_entity_under_existing_unique_id() {
    let db = create_test_db();
    let mut utxo = create_test_utxo(42, 7, 6);

    Utxo::store_and_commit(&db, &utxo).expect("Failed to store initial utxo");

    // Modify the UTXO
    utxo.amount = 1_000_000;
    utxo.datum = String::from("updated_datum");
    utxo.address = String::from("updated_address");

    // Store the modified UTXO (should override the existing one)
    Utxo::store_and_commit(&db, &utxo).expect("Failed to store updated utxo");

    // Retrieve and verify the updated UTXO
    let updated_utxo = Utxo::get(&db.begin_read().unwrap(), &utxo.id).expect("Failed to query updated UTXO");
    assert_eq!(updated_utxo.id, utxo.id);
    assert_eq!(updated_utxo.amount, 1_000_000);
    assert_eq!(updated_utxo.datum, "updated_datum");
    assert_eq!(updated_utxo.address, "updated_address");
}
