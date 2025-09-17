use btc::block_provider::BtcBlockProvider;
use btc::model_v1::{BlockChain, Height};
use btc::rest_client::BtcCBOR;
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use redbit::{info, Storage, WriteTxContext};
use serde_json;
use std::{fs, sync::Arc, time::Duration};

fn block_from_file(size: &str, tx_count: usize) -> bitcoin::Block {
    info!("Getting {} block with {} txs", size, tx_count);
    let path = format!("blocks/{}_block.json", size);
    let file_content = fs::read_to_string(path).expect("Failed to read block file");
    serde_json::from_str(&file_content).expect("Failed to deserialize block from JSON")
}

fn criterion_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let storage = rt.block_on(Storage::temp("btc_benchmark", 1, true)).expect("Failed to create temporary storage");

    let chain = BlockChain::new(Arc::clone(&storage));

    let small_block: BtcCBOR = BtcCBOR { height: Height(135204), raw: bitcoin::consensus::encode::serialize(&block_from_file("small", 29)) };
    let avg_block: BtcCBOR = BtcCBOR { height: Height(217847), raw: bitcoin::consensus::encode::serialize(&block_from_file("avg", 343)) };
    let huge_block: BtcCBOR = BtcCBOR { height: Height(908244), raw: bitcoin::consensus::encode::serialize(&block_from_file("huge", 3713)) };

    info!("Initiating processing");
    let processed_small_block = BtcBlockProvider::process_block_pure(&small_block).expect("Failed to process small_block");
    let processed_avg_block = BtcBlockProvider::process_block_pure(&avg_block).expect("Failed to process avg_block");
    let processed_huge_block = BtcBlockProvider::process_block_pure(&huge_block).expect("Failed to process huge_block");

    info!("Initiating indexing");
    let mut group = c.benchmark_group("btc_chain");
    group.throughput(Throughput::Elements(1));
    group.warm_up_time(Duration::from_millis(50));
    group.measurement_time(Duration::from_millis(300));
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("small_block_processing"), |bencher| {
        bencher.iter(|| BtcBlockProvider::process_block_pure(&small_block).expect("Failed to process small_block"));
    });
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("avg_block_processing"), |bencher| {
        bencher.iter(|| BtcBlockProvider::process_block_pure(&avg_block).expect("Failed to process avg_block"));
    });
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("huge_block_processing"), |bencher| {
        bencher.iter(|| BtcBlockProvider::process_block_pure(&huge_block).expect("Failed to process huge_block"));
    });

    group.sample_size(10);
    let indexing_context = chain.new_indexing_ctx().expect("Failed to create indexing context");
    group.bench_function(BenchmarkId::from_parameter("small_block_persistence"), |bencher| {
        bencher.iter_batched_ref(
            || vec![processed_small_block.clone()], // setup once
            |blocks| {
                let _ = indexing_context.begin_writing().expect("Failed to begin writing");
                chain.store_blocks(&indexing_context, std::mem::take(blocks)).expect("Failed to persist small_block");
                let _ = indexing_context.two_phase_commit().expect("Failed to commit");
            },
            BatchSize::LargeInput,
        );
    });
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("avg_block_persistence"), |bencher| {
        bencher.iter_batched_ref(
            || vec![processed_avg_block.clone()], // setup once
            |blocks| {
                let _ = indexing_context.begin_writing().expect("Failed to begin writing");
                chain.store_blocks(&indexing_context, std::mem::take(blocks)).expect("Failed to persist avg_block");
                let _ = indexing_context.two_phase_commit().expect("Failed to commit");
            },
            BatchSize::LargeInput,
        );
    });
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("huge_block_persistence"), |bencher| {
        bencher.iter_batched_ref(
            || vec![processed_huge_block.clone()], // setup once
            |blocks| {
                let _ = indexing_context.begin_writing().expect("Failed to begin writing");
                chain.store_blocks(&indexing_context, std::mem::take(blocks)).expect("Failed to persist huge_block");
                let _ = indexing_context.two_phase_commit().expect("Failed to commit");
            },
            BatchSize::LargeInput,
        );
    });
    indexing_context.stop_writing().unwrap();

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
