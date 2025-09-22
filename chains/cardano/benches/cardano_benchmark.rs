use cardano::block_provider::CardanoBlockProvider;
use cardano::cardano_client::CardanoCBOR;
use cardano::model_v1::*;
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use pallas_traverse::wellknown::GenesisValues;
use redbit::{info, WriteTxContext};
use std::{fs, sync::Arc, time::Duration};

fn block_from_file(size: &str, tx_count: usize) -> CardanoCBOR {
    info!("Getting {} block with {} txs", size, tx_count);
    let path = format!("blocks/{}_block.cbor", size);
    let bytes = fs::read(&path).expect("Failed to deserialize block from CBOR");
    CardanoCBOR(bytes)
}

fn criterion_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (storage_owner, storage) = rt.block_on(StorageOwner::temp("cardano_benchmark", 1, true)).expect("Failed to open database");

    let chain = BlockChain::new(Arc::clone(&storage));

    let small_block: CardanoCBOR = block_from_file("small", 8);
    let avg_block: CardanoCBOR = block_from_file("avg", 42);
    let huge_block: CardanoCBOR = block_from_file("huge", 322);

    let genesis_value = GenesisValues::mainnet();

    info!("Initiating processing");
    let processed_small_block = CardanoBlockProvider::process_block_pure(&small_block, &genesis_value).expect("Failed to process small_block");
    let processed_avg_block = CardanoBlockProvider::process_block_pure(&avg_block, &genesis_value).expect("Failed to process avg_block");
    let processed_huge_block = CardanoBlockProvider::process_block_pure(&huge_block, &genesis_value).expect("Failed to process huge_block");

    info!("Initiating indexing");
    let mut group = c.benchmark_group("cardano_chain");
    group.throughput(Throughput::Elements(1));
    group.warm_up_time(Duration::from_millis(50));
    group.measurement_time(Duration::from_millis(300));
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("small_block_processing"), |bencher| {
        bencher.iter(|| CardanoBlockProvider::process_block_pure(&small_block, &genesis_value).expect("Failed to process small_block"));
    });
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("avg_block_processing"), |bencher| {
        bencher.iter(|| CardanoBlockProvider::process_block_pure(&avg_block, &genesis_value).expect("Failed to process avg_block"));
    });
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("huge_block_processing"), |bencher| {
        bencher.iter(|| CardanoBlockProvider::process_block_pure(&huge_block, &genesis_value).expect("Failed to process huge_block"));
    });

    group.sample_size(10);
    let indexing_context = chain.new_indexing_ctx().expect("Failed to create indexing context");
    group.bench_function(BenchmarkId::from_parameter("small_block_persistence"), |bencher| {
        bencher.iter_batched_ref(
            || vec![processed_small_block.clone()], // setup once
            |blocks| {
                chain.store_blocks(&indexing_context, std::mem::take(blocks)).expect("Failed to persist block");
            },
            BatchSize::LargeInput,
        );
    });
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("avg_block_persistence"), |bencher| {
        bencher.iter_batched_ref(
            || vec![processed_avg_block.clone()], // setup once
            |blocks| {
                chain.store_blocks(&indexing_context, std::mem::take(blocks)).expect("Failed to persist block");
            },
            BatchSize::LargeInput,
        );
    });
    group.sample_size(10);
    group.bench_function(BenchmarkId::from_parameter("huge_block_persistence"), |bencher| {
        bencher.iter_batched_ref(
            || vec![processed_huge_block.clone()], // setup once
            |blocks| {
                chain.store_blocks(&indexing_context, std::mem::take(blocks)).expect("Failed to persist block");
            },
            BatchSize::LargeInput,
        );
    });
    indexing_context.stop_writing().unwrap();
    drop(storage_owner);
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

