use std::collections::HashSet;
use utxo::*;

#[test]
fn each_entity_should_have_a_default_sample() {
    let block = Block::sample();
    assert_eq!(block.id.0, 0);
    assert_eq!(block.header.id, block.id);
    assert_eq!(block.transactions.len(), 3);
    for (idx, tx) in block.transactions.iter().enumerate() {
        assert_eq!(tx.id.index as usize, idx);
        assert_eq!(tx.id.parent, Height(0));
        assert_eq!(tx.utxos.len(), 3);
        for (idx, utxo) in tx.utxos.iter().enumerate() {
            assert_eq!(utxo.id.index as usize, idx);
            assert_eq!(utxo.id.parent, tx.id);
            assert_eq!(utxo.assets.len(), 3);
            for (idx, asset) in utxo.assets.iter().enumerate() {
                assert_eq!(asset.id.index as usize, idx);
                assert_eq!(asset.id.parent, utxo.id);
            }
        }
    }
}

#[test]
fn it_should_commit_multiple_blocks_in_a_single_tx() {
    let (blocks, multi_tx_db) = init_temp_db("db_test");

    let single_tx_db = empty_temp_db("db_test_2");
    let write_tx = single_tx_db.begin_write().expect("Failed to begin write transaction");
    blocks.iter().for_each(|block| Block::store(&write_tx, block).expect("Failed to persist blocks"));
    write_tx.commit().unwrap();

    let multi_tx_blocks = Block::take(&multi_tx_db.begin_read().unwrap(), 1000).unwrap();
    let single_tx_blocks = Block::take(&single_tx_db.begin_read().unwrap(), 1000).unwrap();

    assert_eq!(multi_tx_blocks.len(), single_tx_blocks.len());
}

#[test]
fn it_should_get_entity_by_unique_id() {
    let (blocks, db) = init_temp_db("db_test");
    let block = blocks.first().unwrap();
    let found_by_id = Block::get(&db.begin_read().unwrap(), &block.id).expect("Failed to query by ID").unwrap();
    assert_eq!(found_by_id.id, block.id);
    assert_eq!(found_by_id.transactions, block.transactions);
    assert_eq!(found_by_id.header, block.header);
}

#[test]
fn it_should_delete_entity_by_unique_id() {
    let (blocks, db) = init_temp_db("db_test");
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
        let inputs_not_found = tx.inputs.iter().any(|input| {
            let input_ref_not_found = InputRef::get(&read_tx, &input.id).unwrap().is_none();
            input_ref_not_found
        });
        tx_not_found && utxos_not_found && inputs_not_found
    });
    assert!(transitive_entities_not_found);
}

#[test]
fn it_should_get_entities_by_index() {
    let (blocks, db) = init_temp_db("db_test");

    let read_tx = db.begin_read().unwrap();
    let transaction = blocks.first().unwrap().transactions.first().unwrap();

    let found_by_hash = Transaction::get_by_hash(&read_tx, &transaction.hash).expect("Failed to query by hash");
    assert_eq!(found_by_hash.len(), 3);
    assert!(found_by_hash.iter().any(|tx| tx.id == transaction.id));
    assert!(found_by_hash.iter().any(|tx| tx.id == transaction.id));
}

#[test]
fn it_should_get_entities_by_index_with_dict() {
    let (blocks, db) = init_temp_db("db_test");

    let read_tx = db.begin_read().unwrap();
    let utxo = blocks.first().unwrap().transactions.first().unwrap().utxos.first().unwrap();

    let found_by_address = Utxo::get_by_address(&read_tx, &utxo.address).expect("Failed to query by address");
    assert_eq!(found_by_address.len(), 3*3);
    assert!(found_by_address.iter().any(|tx| tx.id == utxo.id));
    assert!(found_by_address.iter().any(|tx| tx.id == utxo.id));
}

#[test]
fn it_should_get_entities_by_range_on_index() {
    let (blocks, db) = init_temp_db("db_test");

    let read_tx = db.begin_read().unwrap();

    let from_timestamp = blocks[0].header.timestamp;
    let until_timestamp = blocks[2].header.timestamp;
    let expected_blocks: Vec<BlockHeader> = blocks.into_iter().map(|b|b.header).take(2).collect();
    let unique_timestamps: HashSet<Timestamp> = BlockHeader::take(&read_tx, 1000).unwrap().iter().map(|h| h.timestamp).collect();
    assert_eq!(unique_timestamps.len(), 3);

    let found_by_timestamp_range =
        BlockHeader::range_by_timestamp(&read_tx, &from_timestamp, &until_timestamp).expect("Failed to range by timestamp");
    assert_eq!(found_by_timestamp_range.len(), 2);
    assert_eq!(expected_blocks, found_by_timestamp_range);
}

#[test]
fn it_should_get_entities_by_range_on_pk() {
    let (blocks, db) = init_temp_db("db_test");

    let read_tx = db.begin_read().unwrap();

    let block_pointer_1 = Height(1);
    let block_pointer_2 = Height(2);
    let block_pointer_3 = Height(3);
    let actual_blocks = Block::range(&read_tx, &block_pointer_1, &block_pointer_3).expect("Failed to range by PK");
    let expected_blocks: Vec<Block> = vec![blocks[1].clone(), blocks[2].clone()];

    assert_eq!(expected_blocks.len(), actual_blocks.len());
    assert_eq!(actual_blocks[0].transactions.len(), 3);
    assert_eq!(actual_blocks[1].transactions.len(), 3);
    assert_eq!(expected_blocks, actual_blocks);

    let tx_pointer_1 = TxPointer::from_parent(block_pointer_1).next();
    let tx_pointer_2 = TxPointer::from_parent(block_pointer_2).next().next().next();
    let actual_transactions = Transaction::range(&read_tx, &tx_pointer_1, &tx_pointer_2).expect("Failed to range by PK");
    let mut expected_transactions: Vec<Transaction> = Vec::new();
    expected_transactions.extend(blocks[1].transactions.clone().into_iter().filter(|t| t.id.index >= 1));
    expected_transactions.extend(blocks[2].transactions.clone().into_iter().filter(|t| t.id.index < 3));

    assert_eq!(expected_transactions.len(), actual_transactions.len());
    assert_eq!(expected_transactions, actual_transactions);
    assert!(actual_transactions.iter().all(|t| t.utxos.len() == 3));
}

#[test]
fn it_should_get_related_one_to_many_entities() {
    let (blocks, db) = init_temp_db("db_test");
    let read_tx = db.begin_read().unwrap();
    let block = blocks.first().unwrap();

    let expected_transactions: Vec<Transaction> = block.transactions.clone();
    let transactions = Block::get_transactions(&read_tx, &block.id).expect("Failed to get transactions");

    let expected_utxos: Vec<Utxo> = expected_transactions.iter().flat_map(|t| t.utxos.clone()).collect();
    let utxos: Vec<Utxo> = transactions.iter().flat_map(|t| t.utxos.clone()).collect();

    let expected_input_refs: Vec<InputRef> = expected_transactions.iter().flat_map(|t| t.inputs.clone()).collect();
    let input_refs: Vec<InputRef> = transactions.iter().flat_map(|t| t.inputs.clone()).collect();

    let expected_assets: Vec<Asset> = expected_utxos.iter().flat_map(|u| u.assets.clone()).collect();
    let assets: Vec<Asset> = utxos.iter().flat_map(|u| u.assets.clone()).collect();

    assert_eq!(expected_transactions, transactions);
    assert_eq!(expected_input_refs, input_refs);
    assert_eq!(expected_utxos, utxos);
    assert_eq!(expected_assets, assets);
}

#[test]
fn it_should_get_related_one_to_one_entity() {
    let (blocks, db) = init_temp_db("db_test");
    let read_tx = db.begin_read().unwrap();
    let block = blocks.first().unwrap();

    let expected_header: BlockHeader = block.header.clone();
    let header = Block::get_header(&read_tx, &block.id).expect("Failed to get header");

    assert_eq!(expected_header, header);
}

#[test]
fn it_should_override_entity() {
    let (blocks, db) = init_temp_db("db_test");
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
    let (blocks, db) = init_temp_db("db_test");

    let read_tx = db.begin_read().unwrap();
    let first_block = Block::first(&read_tx).expect("Failed to get first block").unwrap();
    let last_block = Block::last(&read_tx).expect("Failed to get last block").unwrap();

    let first_block_header = BlockHeader::first(&read_tx).expect("Failed to get first header").unwrap();
    let last_block_header = BlockHeader::last(&read_tx).expect("Failed to get last header").unwrap();

    assert_eq!(blocks.first().unwrap().id, first_block.id);
    assert_eq!(blocks.last().unwrap().id, last_block.id);
    
    assert_eq!(blocks.first().unwrap().header, first_block_header);
    assert_eq!(blocks.last().unwrap().header, last_block_header);
}
