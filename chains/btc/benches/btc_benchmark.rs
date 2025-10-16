use btc::block_provider::BtcBlockProvider;
use btc::model_v1::{BlockChain, Input, Utxo};
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use redbit::{assert_sorted, info, Durability, StorageOwner, WriteTxContext};
use std::{sync::Arc, time::Duration};

fn criterion_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (storage_owner, storage) = rt.block_on(StorageOwner::temp("btc_benchmark", 1, true)).expect("Failed to create temporary storage");

    let chain = BlockChain::new(Arc::clone(&storage));

    let (_, small_block) = BtcBlockProvider::block_from_file("small", 135204, 29);
    let (_, avg_block) = BtcBlockProvider::block_from_file("avg", 217847, 343);
    let (_, huge_block) = BtcBlockProvider::block_from_file("huge", 908244, 3713);

    info!("Initiating processing");
    let processed_small_block = BtcBlockProvider::process_block_pure(&small_block).expect("Failed to process small_block");
    let processed_avg_block = BtcBlockProvider::process_block_pure(&avg_block).expect("Failed to process avg_block");
    let processed_huge_block = BtcBlockProvider::process_block_pure(&huge_block).expect("Failed to process huge_block");

    info!("Validating to avoid unexpected write amplification");
    assert_sorted(&processed_small_block.transactions, "Txs", |tx| &tx.id);
    for (idx, tx) in processed_small_block.transactions.iter().enumerate() {
        assert_sorted(&tx.inputs, &format!("Tx[{idx}].inputs"), |inp: &Input| &inp.id);
        assert_sorted(&tx.utxos, &format!("Tx[{idx}].utxos"), |u: &Utxo|   &u.id);
    }

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
    let indexing_context = chain.new_indexing_ctx(Durability::None).expect("Failed to create indexing context");
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
