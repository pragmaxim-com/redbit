use utxo::*;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(std::time::Duration::from_secs(1))
        .warm_up_time(std::time::Duration::from_secs(1))
        .sample_size(10)
}

fn benchmark_persistence(c: &mut Criterion) {
    let db = open_or_create_db("benchmark");
    let blocks = get_blocks(3, 10, 20, 3);

    let mut group = c.benchmark_group("Block");
    group.throughput(Throughput::Elements(1));

    // Persist blocks
    group.bench_function("Block::store_and_commit", |b| {
        b.iter(|| {
            for block in blocks.iter() {
                Block::store_and_commit(&db, block).unwrap();
            }
        })
    });
}

fn benchmark_blocks(c: &mut Criterion) {
    let db = open_or_create_db("benchmark");

    let mut group = c.benchmark_group("Block");
    group.throughput(Throughput::Elements(1));

    let read_tx = db.begin_read().unwrap();
    let first_block = Block::first(&read_tx).unwrap().unwrap();
    let last_block = Block::last(&read_tx).unwrap().unwrap();

    group.bench_function("Block::all", |b| b.iter(|| Block::take(&read_tx, 1000).unwrap()));
    group.bench_function("Block::get", |b| b.iter(|| Block::get(&read_tx, &first_block.id).unwrap()));
    group.bench_function("Block::range", |b| {
        b.iter(|| Block::range(&read_tx, &first_block.id, &last_block.id).unwrap())
    });
    group.bench_function("Block::get_transactions", |b| {
        b.iter(|| Block::get_transactions(&read_tx, &first_block.id).unwrap())
    });
    group.bench_function("Block::get_header", |b| {
        b.iter(|| Block::get_header(&read_tx, &first_block.id).unwrap())
    });
}

fn benchmark_block_headers(c: &mut Criterion) {
    let db = open_or_create_db("benchmark");
    let read_tx = db.begin_read().unwrap();
    let first = BlockHeader::first(&read_tx).unwrap().unwrap();
    let last = BlockHeader::last(&read_tx).unwrap().unwrap();

    let mut group = c.benchmark_group("BlockHeader");
    group.throughput(Throughput::Elements(1));

    group.bench_function("BlockHeader::all", |b| b.iter(|| BlockHeader::take(&read_tx, 1000).unwrap()));
    group.bench_function("BlockHeader::get", |b| {
        b.iter(|| BlockHeader::get(&read_tx, &first.id).unwrap())
    });
    group.bench_function("BlockHeader::range", |b| {
        b.iter(|| BlockHeader::range(&read_tx, &first.id, &last.id).unwrap())
    });
    group.bench_function("BlockHeader::range_by_timestamp", |b| {
        b.iter(|| BlockHeader::range_by_timestamp(&read_tx, &first.timestamp, &last.timestamp).unwrap())
    });
    group.bench_function("BlockHeader::get_by_hash", |b| {
        b.iter(|| BlockHeader::get_by_hash(&read_tx, &first.hash).unwrap())
    });
    group.bench_function("BlockHeader::get_by_timestamp", |b| {
        b.iter(|| BlockHeader::get_by_timestamp(&read_tx, &first.timestamp).unwrap())
    });
    group.bench_function("BlockHeader::get_by_merkle_root", |b| {
        b.iter(|| BlockHeader::get_by_merkle_root(&read_tx, &first.merkle_root).unwrap())
    });
}

fn benchmark_transactions(c: &mut Criterion) {
    let db = open_or_create_db("benchmark");
    let read_tx = db.begin_read().unwrap();
    let first = Transaction::first(&read_tx).unwrap().unwrap();
    let last = Transaction::last(&read_tx).unwrap().unwrap();

    let mut group = c.benchmark_group("Transaction");
    group.throughput(Throughput::Elements(1));

    group.bench_function("Transaction::all", |b| b.iter(|| Transaction::take(&read_tx, 1000).unwrap()));
    group.bench_function("Transaction::get", |b| {
        b.iter(|| Transaction::get(&read_tx, &first.id).unwrap())
    });
    group.bench_function("Transaction::get_by_hash", |b| {
        b.iter(|| Transaction::get_by_hash(&read_tx, &first.hash).unwrap())
    });
    group.bench_function("Transaction::range", |b| {
        b.iter(|| Transaction::range(&read_tx, &first.id, &last.id).unwrap())
    });
    group.bench_function("Transaction::get_utxos", |b| {
        b.iter(|| Transaction::get_utxos(&read_tx, &first.id).unwrap())
    });
}

fn benchmark_utxos(c: &mut Criterion) {
    let db = open_or_create_db("benchmark");
    let read_tx = db.begin_read().unwrap();
    let first = Utxo::first(&read_tx).unwrap().unwrap();
    let last = Utxo::last(&read_tx).unwrap().unwrap();

    let mut group = c.benchmark_group("Utxo");
    group.throughput(Throughput::Elements(1));

    group.bench_function("Utxo::all", |b| b.iter(|| Utxo::take(&read_tx, 1000).unwrap()));
    group.bench_function("Utxo::get", |b| b.iter(|| Utxo::get(&read_tx, &first.id).unwrap()));
    group.bench_function("Utxo::get_by_address", |b| {
        b.iter(|| Utxo::get_by_address(&read_tx, &first.address).unwrap())
    });
    group.bench_function("Utxo::get_by_datum", |b| {
        b.iter(|| Utxo::get_by_datum(&read_tx, &first.datum).unwrap())
    });
    group.bench_function("Utxo::range", |b| {
        b.iter(|| Utxo::range(&read_tx, &first.id, &last.id).unwrap())
    });
    group.bench_function("Utxo::get_assets", |b| {
        b.iter(|| Utxo::get_assets(&read_tx, &first.id).unwrap())
    });
}

fn benchmark_assets(c: &mut Criterion) {
    let db = open_or_create_db("benchmark");
    let read_tx = db.begin_read().unwrap();
    let first = Asset::first(&read_tx).unwrap().unwrap();
    let last = Asset::last(&read_tx).unwrap().unwrap();

    let mut group = c.benchmark_group("Asset");
    group.throughput(Throughput::Elements(1));

    group.bench_function("Asset::all", |b| b.iter(|| Asset::take(&read_tx, 1000).unwrap()));
    group.bench_function("Asset::get", |b| b.iter(|| Asset::get(&read_tx, &first.id).unwrap()));
    group.bench_function("Asset::get_by_name", |b| {
        b.iter(|| Asset::get_by_name(&read_tx, &first.name).unwrap())
    });
    group.bench_function("Asset::get_by_policy_id", |b| {
        b.iter(|| Asset::get_by_policy_id(&read_tx, &first.policy_id).unwrap())
    });
    group.bench_function("Asset::range", |b| {
        b.iter(|| Asset::range(&read_tx, &first.id, &last.id).unwrap())
    });
}

criterion_group!(
    name = benches;
    config = configure_criterion();
    targets = benchmark_persistence, benchmark_blocks, benchmark_block_headers, benchmark_transactions, benchmark_utxos, benchmark_assets
);
criterion_main!(benches);