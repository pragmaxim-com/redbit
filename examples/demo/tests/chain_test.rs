use std::time::Instant;
use demo::block_provider::DemoBlockProvider;
use demo::model_v1::*;
use chain::api::{BlockChainLike, BlockProvider};
use chain::scheduler::Scheduler;
use chain::settings::AppConfig;

#[tokio::test]
async fn chain_sync() {
    let storage = Storage::temp("chain_sync_test", 1, true).expect("Failed to open database");
    let block_provider: Arc<dyn BlockProvider<Block, Block>> = DemoBlockProvider::new(50).expect("Failed to create block provider");
    let chain: Arc<dyn BlockChainLike<Block>> = BlockChain::new(Arc::clone(&storage));
    chain.init().expect("Failed to initialize chain");
    let config = AppConfig::new("config/settings").expect("Failed to load app config");
    let scheduler = Scheduler::new(block_provider, chain.clone());
    let start = Instant::now();
    scheduler.sync(config.indexer.clone()).await.expect("Syncing failed");
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64();
    println!("Demo chain sync took {:.1}s", secs);
    let last_header = chain.get_last_header().unwrap().expect("Failed to get last header");
    assert_eq!(last_header.height, Height(50));
    let read_tx = storage.begin_read().expect("Failed to open database");
    let block_headers = Header::range(&read_tx, &Height(0), &Height(51), None).unwrap();
    assert_eq!(block_headers.len(), 50); // genesis not stored
    let heights: Vec<u32> = block_headers.iter().map(|h| h.height.0).collect();
    assert_eq!(heights, (1..=50).collect::<Vec<u32>>());
    let result = chain.validate_chain().await.expect("Chain validation returned an error");
    assert!(result.is_empty(), "Chain validation failed: {:?}", result);
}

