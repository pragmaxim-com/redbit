#[cfg(all(test, feature = "integration"))]
mod chain_tests {
    use chain::api::BlockProvider;
    use chain::settings::AppConfig;
    use chain::syncer::ChainSyncer;
    use demo::block_provider::DemoBlockProvider;
    use demo::model_v1::*;
    use std::time::Instant;
    use tokio::sync::watch;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn chain_should_sync() {
        let target_height = 200u32;
        let (storage_owner, storage) = StorageOwner::temp("chain_sync_test", 1, true).await.expect("Failed to open database");
        let chain = BlockChain::new(Arc::clone(&storage));
        chain.init().expect("Failed to initialize chain");
        let config = AppConfig::new("config/settings").expect("Failed to load app config");
        let block_provider: Arc<dyn BlockProvider<Block, Block>> = DemoBlockProvider::new(target_height).expect("Failed to create block provider");
        let syncer = ChainSyncer::new(block_provider, chain.clone());
        let start = Instant::now();
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        syncer.sync(&config.indexer, None, shutdown_rx.clone()).await.expect("Syncing failed");
        let elapsed = start.elapsed();
        let secs = elapsed.as_secs_f64();
        println!("Demo chain sync took {:.1}s", secs);
        let last_header = chain.get_last_header().unwrap().expect("Last header must be present");
        assert_eq!(last_header.height, Height(target_height));
        let tx_context = Header::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
        let block_headers = Header::range(&tx_context, Height(0), Height(target_height + 1), None).unwrap();
        let header_near_tip = block_headers.get(target_height as usize - 11).cloned().unwrap();
        assert_eq!(block_headers.len(), target_height as usize); // genesis not stored
        let heights: Vec<u32> = block_headers.iter().map(|h| h.height.0).collect();
        assert_eq!(heights, (1..=target_height).collect::<Vec<u32>>());
        let result = chain.validate_chain(0).await.expect("Chain validation returned an error");
        assert!(result.is_empty(), "Chain validation failed: {:?}", result);

        // sync again
        syncer.sync(&config.indexer, Some(header_near_tip), shutdown_rx).await.expect("Syncing from 40 failed");
        let result = chain.validate_chain(0).await.expect("Chain validation returned an error");
        assert!(result.is_empty(), "Chain validation failed: {:?}", result);

        drop(storage);
        drop(chain);
        drop(syncer);

        for (name, db_arc) in &storage_owner.index_dbs {
            let sc = Arc::strong_count(db_arc);
            if sc != 1 {
                error!("Database {name} still has {sc} strong refs at shutdown");
            }
        }
        drop(storage_owner);
    }

    #[tokio::test]
    async fn chain_should_shutdown() {
        let target_height = 1000u32;

        let (owner, storage) = StorageOwner::temp("chain_sync_shutdown_resume", 1, true).await.expect("Failed to open database");
        let chain = BlockChain::new(Arc::clone(&storage));
        chain.init().expect("Failed to initialize chain");
        let config = AppConfig::new("config/settings").expect("Failed to load app config");

        let block_provider: Arc<dyn BlockProvider<Block, Block>> =
            DemoBlockProvider::new(target_height).expect("Failed to create block provider");
        let syncer = ChainSyncer::new(block_provider, chain.clone());

        // start sync in background so we can interrupt it
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let sync_task = {
            let idx = config.indexer.clone();
            tokio::spawn(async move {
                // NOTE: sync() must be shutdown-aware as we refactored
                syncer.sync(&idx, None, shutdown_rx).await
            })
        };

        sleep(Duration::from_millis(300)).await;

        // trigger shutdown and wait for sync to finish cleanly
        let _ = shutdown_tx.send(true);
        let res = sync_task.await.expect("sync task join");
        assert!(res.is_ok(), "sync should return Ok(()) on shutdown, got {res:?}");

        // exhaustively read headers to ensure no gaps (genesis not stored)
        let tx_context = Header::begin_read_ctx(&storage).expect("read tx ctx");
        let headers = Header::range(&tx_context, Height(0), Height(target_height + 1), None).unwrap();
        assert!(headers.len() > 0 && headers.len() < target_height as usize, "we shutdown somewhere in the middle, got {}", headers.len());

        // database should be consistent; you can validate what got written so far
        let partial = chain.validate_chain(0).await.expect("validate_chain");
        assert!(partial.is_empty(), "chain invalid after shutdown");

        // teardown in correct order (weak-view first, then owner to trigger Database::drop)
        drop(storage);
        drop(chain);
        for (name, db_arc) in &owner.index_dbs {
            let sc = Arc::strong_count(db_arc);
            if sc != 1 {
                error!("Database {name} still has {sc} strong refs at shutdown");
            }
        }
        drop(owner);
    }
}
