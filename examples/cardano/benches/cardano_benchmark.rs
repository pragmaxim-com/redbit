use std::{fs, sync::Arc, time::Duration};
use chain::api::BlockChainLike;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use pallas::network::miniprotocols::Point;
use pallas_traverse::wellknown::GenesisValues;
use serde_json;
use tokio::runtime::Runtime;
use cardano::block_provider::CardanoBlockProvider;
use cardano::cardano_client::{CardanoClient, CBOR};
use cardano::config::CardanoConfig;
use cardano::model_v1::{Block, BlockChain};
use redbit::{info, Storage};

fn block_from_file(size: &str, tx_count: usize) -> CBOR {
    info!("Getting {} block with {} txs", size, tx_count);
    let path = format!("blocks/{}_block.json", size);
    let file_content = fs::read_to_string(path).expect("Failed to read block file");
    serde_json::from_str(&file_content).expect("Failed to deserialize block from JSON")
}

fn criterion_benchmark(c: &mut Criterion) {
    let storage = Storage::temp("btc_benchmark", 1, true).expect("Failed to open database");
    let cardano_config = CardanoConfig::new("config/cardano").expect("Failed to load Cardano configuration");

    let chain: Arc<dyn BlockChainLike<Block>> = BlockChain::new(Arc::clone(&storage));

    let rt = Runtime::new().unwrap();
    let cardano_client: CardanoClient = rt.block_on(CardanoClient::new(&cardano_config));

    let point = Point::new(164130390 as u64, hex::decode("a3aa6f442e2ad8aecf6d4b675bbc933f870677ace79686e580389fe27cee4aa8").unwrap());
    let cbor = rt.block_on(cardano_client.get_block_by_point(point)).expect("Failed to get block by point");
    // convert to json
    let cbor_json = serde_json::to_string(&cbor).expect("Failed to serialize CBOR to JSON");
    // write to file
    fs::write("blocks/small_block.json", cbor_json).expect("Failed to write small block to file");
    return;
    let small_block: CBOR = block_from_file("small", 29);
    let avg_block: CBOR = block_from_file("avg", 343);
    let huge_block: CBOR = block_from_file("huge", 3713);
    let genesis_value = GenesisValues::mainnet();

    info!("Initiating processing");
    let processed_small_block = CardanoBlockProvider::process_block_pure(&small_block, &genesis_value).expect("Failed to process small_block");
    let processed_avg_block = CardanoBlockProvider::process_block_pure(&avg_block, &genesis_value).expect("Failed to process avg_block");
    let processed_huge_block = CardanoBlockProvider::process_block_pure(&huge_block, &genesis_value).expect("Failed to process huge_block");

    info!("Initiating indexing");
    let mut group = c.benchmark_group("btc_chain");
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

