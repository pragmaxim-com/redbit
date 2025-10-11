use std::{fs, sync::Arc, time::Duration};

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use ergo::block_provider::ErgoBlockProvider;
use ergo::ergo_client::ErgoCBOR;
use ergo::model_v1::{BlockChain, Input, Utxo};
use redbit::{assert_sorted, info, StorageOwner, WriteTxContext};

fn block_from_file(size: &str, tx_count: usize) -> ErgoCBOR {
    info!("Getting {} block with {} txs", size, tx_count);
    let path = format!("blocks/{}_block.json", size);
    let file_content = fs::read(path).expect("Failed to read block file");
    ErgoCBOR(file_content)
}

fn criterion_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (storage_owner, storage) = rt.block_on(StorageOwner::temp("ergo_benchmark", 1, true)).expect("Failed to open database");
    let chain = BlockChain::new(Arc::clone(&storage));

    let small_block: ErgoCBOR = block_from_file("small", 8);
    let avg_block: ErgoCBOR = block_from_file("avg", 49);
    let huge_block: ErgoCBOR = block_from_file("huge", 320);

    info!("Initiating processing");
    let processed_small_block = ErgoBlockProvider::process_block_pure(&small_block).expect("Failed to process small_block");
    let processed_avg_block = ErgoBlockProvider::process_block_pure(&avg_block).expect("Failed to process avg_block");
    let processed_huge_block = ErgoBlockProvider::process_block_pure(&huge_block).expect("Failed to process huge_block");

    info!("Validating to avoid unexpected write amplification");
    assert_sorted(&processed_small_block.transactions, "Txs", |tx| &tx.id);
    for (idx, tx) in processed_small_block.transactions.iter().enumerate() {
        assert_sorted(&tx.inputs, &format!("Tx[{idx}].inputs"), |inp: &Input| &inp.id);
        assert_sorted(&tx.utxos, &format!("Tx[{idx}].utxos"), |u: &Utxo|   &u.id);
    }

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

