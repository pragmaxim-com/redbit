#[cfg(all(test, not(feature = "integration")))]
mod entity_tests {
    use demo::model_v1::*;
    use std::collections::HashSet;
    use redbit::storage::init::StorageOwner;

    async fn init_temp_storage(name: &str, db_cache_size_gb: u8) -> (Vec<Block>, StorageOwner, Arc<Storage>) {
        let (storage_owner, storage) = StorageOwner::temp(name, db_cache_size_gb, true).await.unwrap();
        let blocks = Block::sample_many(3);
        let tx_context = Block::begin_write_ctx(&storage).unwrap();
        Block::store_many(&tx_context, blocks.clone()).expect("Failed to persist blocks");
        tx_context.commit_and_close_ctx().unwrap();
        (blocks, storage_owner, storage)
    }

    #[test]
    fn each_entity_should_have_a_default_sample() {
        let block = Block::sample();
        assert_eq!(block.height.0, 0);
        assert_eq!(block.header.height, block.height);
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

    #[tokio::test]
    async fn it_should_commit_multiple_blocks_in_a_single_tx() {
        let (blocks, _multi_tx_storage_owner, multi_tx_storage) = init_temp_storage("db_test", 0).await;

        let (_storage_owner, single_tx_db) = StorageOwner::temp("db_test_2", 0, true).await.unwrap();
        let tx_context = Block::begin_write_ctx(&single_tx_db).unwrap();
        blocks.into_iter().for_each(|block| Block::store(&tx_context, block).expect("Failed to persist blocks"));
        tx_context.commit_and_close_ctx().unwrap();

        let block_tx = Block::begin_read_ctx(&multi_tx_storage).unwrap();
        let multi_tx_blocks = Block::take(&block_tx, 100).unwrap();
        let single_tx_blocks = Block::take(&block_tx, 100).unwrap();

        assert_eq!(multi_tx_blocks.len(), single_tx_blocks.len());
    }

    #[tokio::test]
    async fn it_should_get_entity_by_unique_id() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;
        let block = blocks.first().unwrap();
        let block_tx = Block::begin_read_ctx(&storage).unwrap();
        let found_by_id = Block::get(&block_tx, block.height).expect("Failed to query by ID").unwrap();
        assert_eq!(found_by_id.height, block.height);
        assert_eq!(found_by_id.transactions, block.transactions);
        assert_eq!(found_by_id.header, block.header);
    }

    #[tokio::test]
    async fn it_should_delete_entity_by_unique_id() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;
        let block = blocks.first().unwrap();
        let block_tx = Block::begin_read_ctx(&storage).unwrap();
        let found_by_id = Block::get(&block_tx, block.height).expect("Failed to query by ID").unwrap();
        assert_eq!(found_by_id.height, block.height);

        Block::remove(Arc::clone(&storage), block.height).expect("Failed to delete by ID");

        let block_tx = Block::begin_read_ctx(&storage).unwrap();

        let block_not_found = Block::get(&block_tx, block.height).expect("Failed to query by ID after deletion").is_none();
        assert!(block_not_found);

        let transitive_entities_not_found = block.transactions.iter().any(|tx| {
            let tx_not_found = Transaction::get(&block_tx.transactions, tx.id).unwrap().is_none();
            let utxos_not_found = tx.utxos.iter().any(|utxo| {
                let utxo_not_found = Utxo::get(&block_tx.transactions.utxos, utxo.id).unwrap().is_none();
                let assets_not_found = utxo.assets.iter().any(|asset| Asset::get(&block_tx.transactions.utxos.assets, asset.id).unwrap().is_none());
                utxo_not_found && assets_not_found
            });
            tx_not_found && utxos_not_found
        });
        assert!(transitive_entities_not_found);
    }

    #[tokio::test]
    async fn it_should_stream_entities_by_index() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;

        let transaction_tx = Transaction::begin_read_ctx(&storage).unwrap();
        let transaction = blocks.first().unwrap().transactions.first().unwrap();

        let found_by_hash = Transaction::stream_by_hash(transaction_tx, transaction.hash.clone(), None).unwrap().try_collect::<Vec<Transaction>>().await.unwrap();
        assert_eq!(found_by_hash.len(), 3);
        assert!(found_by_hash.iter().any(|tx| tx.id == transaction.id));
        assert!(found_by_hash.iter().any(|tx| tx.id == transaction.id));
    }

    #[tokio::test]
    async fn it_should_stream_entities_by_index_with_dict() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;

        let utxo_tx = Utxo::begin_read_ctx(&storage).unwrap();
        let utxo = blocks.first().unwrap().transactions.first().unwrap().utxos.first().unwrap();

        let found_by_address = Utxo::stream_by_address(utxo_tx, utxo.address.clone(), None).unwrap().try_collect::<Vec<Utxo>>().await.unwrap();
        assert_eq!(found_by_address.len(), 9);
        assert!(found_by_address.iter().any(|tx| tx.id == utxo.id));
        assert!(found_by_address.iter().any(|tx| tx.id == utxo.id));
    }

    #[tokio::test]
    async fn store_many_utxos() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;
        let all_utxos = blocks.iter().flat_map(|b| b.transactions.iter().flat_map(|t| t.utxos.clone())).collect::<Vec<Utxo>>();
        let tx_context = Utxo::begin_write_ctx(&storage).unwrap();
        Utxo::store_many(&tx_context, all_utxos).expect("Failed to store UTXO");
        tx_context.commit_and_close_ctx().expect("Failed to flush transaction context");
    }

    #[tokio::test]
    async fn it_should_stream_entities_by_range_on_index() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;

        let header_tx = Header::begin_read_ctx(&storage).unwrap();

        let from_timestamp = blocks[0].header.timestamp;
        let until_timestamp = blocks[2].header.timestamp;
        let expected_blocks: Vec<Header> = blocks.into_iter().map(|b|b.header).take(2).collect();
        let unique_timestamps: HashSet<Timestamp> = Header::take(&header_tx, 100).unwrap().iter().map(|h| h.timestamp).collect();
        assert_eq!(unique_timestamps.len(), 3);

        let found_by_timestamp_range =
            Header::stream_range_by_timestamp(header_tx, from_timestamp, until_timestamp, None).unwrap().try_collect::<Vec<Header>>().await.unwrap();
        assert_eq!(found_by_timestamp_range.len(), 2);
        assert_eq!(expected_blocks, found_by_timestamp_range);
    }

    #[tokio::test]
    async fn it_should_get_entities_by_index() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;

        let transaction_tx = Transaction::begin_read_ctx(&storage).unwrap();
        let transaction = blocks.first().unwrap().transactions.first().unwrap();

        let found_by_hash = Transaction::get_by_hash(&transaction_tx, &transaction.hash).expect("Failed to query by hash");
        assert_eq!(found_by_hash.len(), 3);
        assert!(found_by_hash.iter().any(|tx| tx.id == transaction.id));
        assert!(found_by_hash.iter().any(|tx| tx.id == transaction.id));
    }

    #[tokio::test]
    async fn it_should_get_entities_by_index_with_dict() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;

        let utxo_tx = Utxo::begin_read_ctx(&storage).unwrap();
        let utxo = blocks.first().unwrap().transactions.first().unwrap().utxos.first().unwrap();

        let found_by_address = Utxo::get_by_address(&utxo_tx, &utxo.address).expect("Failed to query by address");
        assert_eq!(found_by_address.len(), 9);
        assert!(found_by_address.iter().any(|tx| tx.id == utxo.id));
        assert!(found_by_address.iter().any(|tx| tx.id == utxo.id));
    }


    #[tokio::test]
    async fn it_should_get_entities_by_range_on_pk() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;

        let block_tx = Block::begin_read_ctx(&storage).unwrap();

        let height_1 = Height(1);
        let height_2 = Height(2);
        let height_3 = Height(3);
        let actual_blocks = Block::range(&block_tx, height_1, height_3, None).expect("Failed to range by PK");
        let expected_blocks: Vec<Block> = vec![blocks[1].clone(), blocks[2].clone()];

        assert_eq!(expected_blocks.len(), actual_blocks.len());
        assert_eq!(actual_blocks[0].transactions.len(), 3);
        assert_eq!(actual_blocks[1].transactions.len(), 3);
        assert_eq!(expected_blocks, actual_blocks);

        let tx_pointer_1 = BlockPointer::from_parent(height_1, 1);
        let tx_pointer_2 = BlockPointer::from_parent(height_2, 3);
        let actual_transactions = Transaction::range(&block_tx.transactions, tx_pointer_1, tx_pointer_2, None).expect("Failed to range by PK");
        let mut expected_transactions: Vec<Transaction> = Vec::new();
        expected_transactions.extend(blocks[1].transactions.clone().into_iter().filter(|t| t.id.index >= 1));
        expected_transactions.extend(blocks[2].transactions.clone().into_iter().filter(|t| t.id.index < 3));

        assert_eq!(expected_transactions.len(), actual_transactions.len());
        assert_eq!(expected_transactions, actual_transactions);
        assert!(actual_transactions.iter().all(|t| t.utxos.len() == 3));
    }

    #[tokio::test]
    async fn it_should_get_related_one_to_many_entities() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;
        let transaction_tx = Transaction::begin_read_ctx(&storage).unwrap();
        let block = blocks.first().unwrap();

        let expected_transactions: Vec<Transaction> = block.transactions.clone();
        let transactions = Block::get_transactions(&transaction_tx, block.height).expect("Failed to get transactions");

        let expected_utxos: Vec<Utxo> = expected_transactions.iter().flat_map(|t| t.utxos.clone()).collect();
        let utxos: Vec<Utxo> = transactions.iter().flat_map(|t| t.utxos.clone()).collect();

        let expected_assets: Vec<Asset> = expected_utxos.iter().flat_map(|u| u.assets.clone()).collect();
        let assets: Vec<Asset> = utxos.iter().flat_map(|u| u.assets.clone()).collect();

        assert_eq!(expected_transactions, transactions);
        assert_eq!(expected_utxos, utxos);
        assert_eq!(expected_assets, assets);
    }

    #[tokio::test]
    async fn it_should_get_related_one_to_one_entity() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;
        let header_tx = Header::begin_read_ctx(&storage).unwrap();
        let block = blocks.first().unwrap();

        let expected_header: Header = block.header.clone();
        let header = Block::get_header(&header_tx, block.height).expect("Failed to get header");

        assert_eq!(expected_header, header);
    }

    #[tokio::test]
    async fn it_should_override_entity() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;
        let block_tx = Block::begin_read_ctx(&storage).unwrap();
        let block = blocks.first().cloned().unwrap();
        let block_height = block.height;

        let loaded_block = Block::get(&block_tx, block_height).expect("Failed to get by ID").unwrap();

        assert_eq!(&block, &loaded_block);

        Block::persist(Arc::clone(&storage), block.clone()).expect("Failed to delete by ID");
        let loaded_block2 = Block::get(&block_tx, block_height).expect("Failed to get by ID").unwrap();

        assert_eq!(&block, &loaded_block2);
    }

    #[tokio::test]
    async fn it_should_get_first_and_last_entity() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;

        let block_tx = Block::begin_read_ctx(&storage).unwrap();
        let first_block = Block::first(&block_tx).expect("Failed to get first block").unwrap();
        let last_block = Block::last(&block_tx).expect("Failed to get last block").unwrap();

        let first_block_header = Header::first(&block_tx.header).expect("Failed to get first header").unwrap();
        let last_block_header = Header::last(&block_tx.header).expect("Failed to get last header").unwrap();

        assert_eq!(blocks.first().unwrap().height, first_block.height);
        assert_eq!(blocks.last().unwrap().height, last_block.height);

        assert_eq!(blocks.first().unwrap().header, first_block_header);
        assert_eq!(blocks.last().unwrap().header, last_block_header);
    }
}
