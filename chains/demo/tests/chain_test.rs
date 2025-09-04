use chain::api::{BlockChainLike, BlockProvider};
use chain::settings::AppConfig;
use chain::syncer::ChainSyncer;
use demo::block_provider::DemoBlockProvider;
use demo::model_v1::*;
use std::time::Instant;
use tokio::sync::watch;
use demo::block_chain::BlockChain;

#[tokio::test]
async fn test_chain_sync() {
    let target_height = 200u32;
    let storage = Storage::temp("chain_sync_test", 1, true).expect("Failed to open database");
    let chain: Arc<dyn BlockChainLike<Block>> = BlockChain::new(Arc::clone(&storage));
    chain.init().expect("Failed to initialize chain");
    let config = AppConfig::new("config/settings").expect("Failed to load app config");
    let block_provider: Arc<dyn BlockProvider<Block, Block>> = DemoBlockProvider::new(target_height).expect("Failed to create block provider");
    let syncer = ChainSyncer::new(block_provider, chain.clone());
    let start = Instant::now();
    let (_, shutdown_rx) = watch::channel(false);
    syncer.sync(&config.indexer, None, shutdown_rx.clone()).await.expect("Syncing failed");
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64();
    println!("Demo chain sync took {:.1}s", secs);
    let last_header = chain.get_last_header().unwrap().expect("Last header must be present");
    assert_eq!(last_header.height, Height(target_height));
    let read_tx = storage.db.begin_read().expect("Failed to open database");
    let tx_context = Header::begin_read_tx(&read_tx).expect("Failed to begin read transaction context");
    let block_headers = Header::range(&tx_context, &Height(0), &Height(target_height + 1), None).unwrap();
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
}

