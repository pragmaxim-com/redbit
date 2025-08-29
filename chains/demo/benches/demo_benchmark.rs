use chain::api::{BlockChainLike, BlockProvider};
use std::{sync::Arc, time::Duration};

use chain::settings::AppConfig;
use chain::syncer::ChainSyncer;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use demo::block_provider::DemoBlockProvider;
use demo::model_v1::Block;
use redbit::{info, Storage};
use tokio::runtime::Runtime;
use tokio::sync::watch;
use demo::block_chain::BlockChain;

fn criterion_benchmark(c: &mut Criterion) {
    let storage = Storage::temp("demo_benchmark", 1, true).expect("Failed to open database");

    let block_provider: Arc<dyn BlockProvider<Block, Block>> = DemoBlockProvider::new(10).expect("Failed to create block provider");
    let chain: Arc<dyn BlockChainLike<Block>> = BlockChain::new(Arc::clone(&storage));
    chain.init().expect("Failed to initialize chain");
    let syncer = ChainSyncer::new(block_provider, chain.clone());
    let mut config = AppConfig::new("config/settings").expect("Failed to load app config");
    config.indexer.fork_detection_heights = 5;

    info!("Initiating syncing");
    let mut group = c.benchmark_group("demo_chain");
    group.throughput(Throughput::Elements(1));
    group.warm_up_time(Duration::from_millis(50));
    group.measurement_time(Duration::from_millis(300));
    group.sample_size(10);

    let rt = Runtime::new().unwrap();
    let (_, shutdown_rx) = watch::channel(false);
    group.bench_function(BenchmarkId::from_parameter("syncing"), |bencher| {
        bencher.to_async(&rt).iter(|| async {
            syncer.sync(&config.indexer, None, shutdown_rx.clone()).await.expect("Syncing failed"); // syncing is ~ as fast as deleting, which is good
            chain.delete()
        })
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

