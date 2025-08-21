use chain::api::BlockChainLike;
use std::{fs, sync::Arc, time::Duration};

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use ergo::block_provider::ErgoBlockProvider;
use ergo::model_v1::{Block, BlockChain};
use ergo_lib::chain::block::FullBlock;
use redbit::{info, serde_json, Storage};

fn block_from_file(size: &str, tx_count: usize) -> FullBlock {
    info!("Getting {} block with {} txs", size, tx_count);
    let path = format!("blocks/{}_block.json", size);
    let file_content = fs::read_to_string(path).expect("Failed to read block file");
    serde_json::from_str(&file_content).expect("Failed to deserialize block from JSON")
}

fn criterion_benchmark(c: &mut Criterion) {
    let storage = Storage::temp("ergo_benchmark", 1, true).expect("Failed to open database");
    let chain: Arc<dyn BlockChainLike<Block>> = BlockChain::new(Arc::clone(&storage));

    let small_block: FullBlock = block_from_file("small", 8);
    let avg_block: FullBlock = block_from_file("avg", 49);
    let huge_block: FullBlock = block_from_file("huge", 320);

    info!("Initiating processing");
    let processed_small_block = ErgoBlockProvider::process_block_pure(&small_block).expect("Failed to process small_block");
    let processed_avg_block = ErgoBlockProvider::process_block_pure(&avg_block).expect("Failed to process avg_block");
    let processed_huge_block = ErgoBlockProvider::process_block_pure(&huge_block).expect("Failed to process huge_block");

    info!("Initiating indexing");
    let mut group = c.benchmark_group("ergo_chain");
    group.throughput(Throughput::Elements(1));
    group.warm_up_time(Duration::from_millis(50));
    group.measurement_time(Duration::from_millis(300));
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("small_block_processing"), |bencher| {
        bencher.iter(|| ErgoBlockProvider::process_block_pure(&small_block).expect("Failed to process small_block"));
    });
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("avg_block_processing"), |bencher| {
        bencher.iter(|| ErgoBlockProvider::process_block_pure(&avg_block).expect("Failed to process avg_block"));
    });
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("huge_block_processing"), |bencher| {
        bencher.iter(|| ErgoBlockProvider::process_block_pure(&huge_block).expect("Failed to process huge_block"));
    });

    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("small_block_persistence"), |bencher| {
        bencher.iter_batched_ref(
            || vec![processed_small_block.clone()], // setup once
            |blocks| {
                chain
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
                chain
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
                chain
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

