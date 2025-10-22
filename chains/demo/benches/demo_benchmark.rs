use chain::api::BlockProvider;
use std::{sync::Arc, time::Duration};

use chain::settings::AppConfig;
use chain::syncer::ChainSyncer;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use demo::block_provider::DemoBlockProvider;
use demo::model_v1::*;
use redbit::info;
use tokio::runtime::Runtime;
use tokio::sync::watch;

fn criterion_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (storage_owner, storage) = rt.block_on(StorageOwner::temp("demo_benchmark", 1, true)).unwrap();

    let block_provider: Arc<dyn BlockProvider<Block, Block>> = DemoBlockProvider::new(10).expect("Failed to create block provider");
    let chain = BlockChain::new(Arc::clone(&storage));
    chain.init().expect("Failed to initialize chain");
    let syncer = ChainSyncer::new(block_provider, chain.clone());
    let mut config: AppConfig = chain_config::load_config("config/settings", "REDBIT").expect("Failed to load Redbit settings");
    config.indexer.fork_detection_heights = 5;

    info!("Initiating syncing");
    let mut group = c.benchmark_group("demo_chain");
    group.throughput(Throughput::Elements(1));
    group.warm_up_time(Duration::from_millis(50));
    group.measurement_time(Duration::from_millis(300));
    group.sample_size(10);

    let (_, shutdown_rx) = watch::channel(false);
    group.bench_function(BenchmarkId::from_parameter("syncing"), |bencher| {
        bencher.to_async(&rt).iter(|| async {
            syncer.sync(&config.indexer, None, shutdown_rx.clone()).await.expect("Syncing failed"); // syncing is ~ as fast as deleting, which is good
            chain.delete()
        })
    });

    group.bench_function(BenchmarkId::from_parameter("indexing_context"), |bencher| {
        bencher.iter(|| {
                let ctx = chain.new_indexing_ctx().expect("Failed to create indexing context");
                ctx.begin_writing(Durability::None).unwrap();
                ctx.two_phase_commit_and_close().unwrap();
            }
        );
    });
    drop(storage_owner);
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

