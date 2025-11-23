#[cfg(all(test, feature = "integration"))]
mod manual_chain_tests {
    use chain::api::BlockProvider;
    use chain::settings::AppConfig;
    use chain::syncer::ChainSyncer;
    use demo::block_provider::DemoBlockProvider;
    use demo::manual_chain::build_block_chain_auto;
    use demo::model_v1::*;
    use std::sync::Arc;
    use std::time::Instant;
    use tokio::sync::watch;

    #[tokio::test]
    async fn manual_chain_store_blocks_basic() {
        let (owner, storage) = StorageOwner::temp("manual_chain_store_blocks_basic", 1, true).await.expect("storage");
        let chain = build_block_chain_auto(Arc::clone(&storage));
        chain.init().expect("init");
        let ctx = chain.new_indexing_ctx().expect("ctx");
        let blocks = Block::sample_many(Height(1), 3);
        chain.store_blocks(&ctx, blocks, Durability::None).expect("store");
        ctx.stop_writing().expect("stop");
        let last_header = chain.get_last_header().expect("get_last_header").expect("present");
        assert_eq!(last_header.height, Height(3));
        drop(chain);
        drop(storage);
        owner.assert_last_refs();
        drop(owner);
    }

    #[tokio::test]
    async fn manual_chain_should_sync() {
        let target_height = 50u32;
        let (storage_owner, storage) = StorageOwner::temp("manual_chain_sync_test", 1, true).await.expect("Failed to open database");
        let chain = build_block_chain_auto(Arc::clone(&storage));
        chain.init().expect("Failed to initialize manual chain");
        let config: AppConfig = chain_config::load_config("config/settings", "REDBIT").expect("Failed to load Redbit settings");
        let block_provider: Arc<dyn BlockProvider<Block, Block>> =
            DemoBlockProvider::for_height(target_height, config.indexer.max_entity_buffer_kb_size).expect("Failed to create block provider");
        let syncer = ChainSyncer::new(block_provider, chain.clone());
        let start = Instant::now();
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        syncer.sync(&config.indexer, None, shutdown_rx.clone()).await.expect("Syncing failed");
        let elapsed = start.elapsed();
        let secs = elapsed.as_secs_f64();
        println!("Manual demo chain sync took {:.1}s", secs);

        // validate tip and header coverage (genesis not stored)
        let last_header = chain.get_last_header().unwrap().expect("Last header must be present");
        assert_eq!(last_header.height, Height(target_height));
        let tx_context = Header::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
        let block_headers = Header::range(&tx_context, Height(0), Height(target_height + 1), None).unwrap();
        assert_eq!(block_headers.len(), target_height as usize);

        // no validation errors expected
        let result = chain.validate_chain(0).await.expect("Chain validation returned an error");
        assert!(result.is_empty(), "Manual chain validation failed: {:?}", result);

        drop(storage);
        drop(chain);
        storage_owner.assert_last_refs();
        drop(storage_owner);
    }
}
