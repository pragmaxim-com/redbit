use syncer::api::{BlockPersistence, BlockProvider};
use syncer::{info, settings};
use std::{env, fs, sync::Arc, time::Duration};

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use btc::block_persistence::BtcBlockPersistence;
use btc::block_provider::BtcBlockProvider;
use btc::btc_client::{BtcBlock, BtcClient};
use btc::config::BitcoinConfig;
use btc::model_v1::{Block, Height};
use btc::storage;

fn criterion_benchmark(c: &mut Criterion) {
    let app_config = settings::AppConfig::new("config/settings").unwrap();
    let btc_config = BitcoinConfig::new("config/bitcoin").expect("Failed to load Bitcoin configuration");
    let db_name = format!("{}/{}", "btc_indexer", "benchmark");
    let db_path = env::temp_dir().join(&db_name);
    if db_path.exists() {
        info!("Removing existing database directory: {}", db_path.display());
        fs::remove_dir_all(&db_path).unwrap();
    }
    let storage = Arc::new(storage::get_storage(db_path, 1).expect("Failed to open database"));

    let btc_client = Arc::new(BtcClient::new(&btc_config).expect("Failed to create Bitcoin client"));
    let fetching_par: usize = app_config.indexer.fetching_parallelism.clone().into();
    let block_provider: Arc<dyn BlockProvider<BtcBlock, Block>> =
        Arc::new(BtcBlockProvider::new(btc_client.clone(), fetching_par).expect("Failed to create block provider"));
    let block_persistence: Arc<dyn BlockPersistence<Block>> = Arc::new(BtcBlockPersistence { storage: Arc::clone(&storage) });

    info!("Getting small block with 29 txs");
    let small_block = btc_client.get_block_by_height(Height(135204)).unwrap();
    info!("Getting avg block with 343 txs");
    let avg_block = btc_client.get_block_by_height(Height(217847)).unwrap();
    info!("Getting huge block with 3713 txs");
    let huge_block = btc_client.get_block_by_height(Height(908244)).unwrap();

    info!("Initiating processing");
    let processed_huge_block = block_provider.process_block(&huge_block).expect("Failed to process huge_block");
    let processed_avg_block = block_provider.process_block(&avg_block).expect("Failed to process avg_block");
    let processed_small_block = block_provider.process_block(&small_block).expect("Failed to process small_block");

    info!("Initiating indexing");
    let mut group = c.benchmark_group("persistence");
    group.throughput(Throughput::Elements(1));
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_millis(1000));
    group.sample_size(85);
    group.bench_function(BenchmarkId::from_parameter("small_block_processing"), |bencher| {
        bencher.iter(|| block_provider.process_block(&small_block).expect("Failed to process small_block"));
    });
    group.sample_size(60);
    group.bench_function(BenchmarkId::from_parameter("avg_block_processing"), |bencher| {
        bencher.iter(|| block_provider.process_block(&avg_block).expect("Failed to process avg_block"));
    });
    group.sample_size(45);
    group.bench_function(BenchmarkId::from_parameter("huge_block_processing"), |bencher| {
        bencher.iter(|| block_provider.process_block(&huge_block).expect("Failed to process huge_block"));
    });

    group.sample_size(20);
    group.bench_function(BenchmarkId::from_parameter("small_block_persistence"), |bencher| {
        bencher.iter_batched_ref(
            || vec![processed_small_block.clone()], // setup once
            |blocks| {
                block_persistence
                    .store_blocks(std::mem::take(blocks))
                    .expect("Failed to persist small_block");
            },
            BatchSize::LargeInput,
        );
    });
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("avg_block_persistence"), |bencher| {
        bencher.iter_batched_ref(
            || vec![processed_avg_block.clone()], // setup once
            |blocks| {
                block_persistence
                    .store_blocks(std::mem::take(blocks))
                    .expect("Failed to persist avg_block");
            },
            BatchSize::LargeInput,
        );
    });
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("huge_block_persistence"), |bencher| {
        bencher.iter_batched_ref(
            || vec![processed_huge_block.clone()], // setup once
            |blocks| {
                block_persistence
                    .store_blocks(std::mem::take(blocks))
                    .expect("Failed to persist huge_block");
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
