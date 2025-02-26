use utxo::*;
use redb::Database;
use std::env::temp_dir;
use criterion::{criterion_group, criterion_main, Criterion};

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(std::time::Duration::from_secs(1))
        .warm_up_time(std::time::Duration::from_secs(1))
        .sample_size(10)
}

fn setup_db() -> Database {
    let db_path = temp_dir().join("benchmark_db.redb");
    Database::create(db_path).expect("Failed to create database")
}


fn benchmark_blocks(c: &mut Criterion) {
    let db = setup_db();
    let blocks = get_blocks(1, 5, 5, 3);
    let block = blocks.first().unwrap();

    // Persist blocks
    c.bench_function("Block::store_and_commit", |b| {
        b.iter(|| {
            Block::store_and_commit(&db, block).unwrap();
        })
    });

    let read_tx = db.begin_read().unwrap();
    let first_block = Block::first(&read_tx).unwrap().unwrap();
    let last_block = Block::last(&read_tx).unwrap().unwrap();

    c.bench_function("Block::all", |b| b.iter(|| Block::all(&read_tx).unwrap()));
    c.bench_function("Block::get", |b| b.iter(|| Block::get(&read_tx, &first_block.id).unwrap()));
    c.bench_function("Block::range", |b| {
        b.iter(|| Block::range(&read_tx, &first_block.id, &last_block.id).unwrap())
    });
    c.bench_function("Block::get_transactions", |b| {
        b.iter(|| Block::get_transactions(&read_tx, &first_block.id).unwrap())
    });
    c.bench_function("Block::get_header", |b| {
        b.iter(|| Block::get_header(&read_tx, &first_block.id).unwrap())
    });
}

fn benchmark_block_headers(c: &mut Criterion) {
    let db = setup_db();
    let read_tx = db.begin_read().unwrap();
    let first = BlockHeader::first(&read_tx).unwrap().unwrap();
    let last = BlockHeader::last(&read_tx).unwrap().unwrap();

    c.bench_function("BlockHeader::all", |b| b.iter(|| BlockHeader::all(&read_tx).unwrap()));
    c.bench_function("BlockHeader::get", |b| {
        b.iter(|| BlockHeader::get(&read_tx, &first.id).unwrap())
    });
    c.bench_function("BlockHeader::range", |b| {
        b.iter(|| BlockHeader::range(&read_tx, &first.id, &last.id).unwrap())
    });
    c.bench_function("BlockHeader::range_by_timestamp", |b| {
        b.iter(|| BlockHeader::range_by_timestamp(&read_tx, &first.timestamp, &last.timestamp).unwrap())
    });
    c.bench_function("BlockHeader::get_by_hash", |b| {
        b.iter(|| BlockHeader::get_by_hash(&read_tx, &first.hash).unwrap())
    });
    c.bench_function("BlockHeader::get_by_timestamp", |b| {
        b.iter(|| BlockHeader::get_by_timestamp(&read_tx, &first.timestamp).unwrap())
    });
    c.bench_function("BlockHeader::get_by_merkle_root", |b| {
        b.iter(|| BlockHeader::get_by_merkle_root(&read_tx, &first.merkle_root).unwrap())
    });
}

fn benchmark_transactions(c: &mut Criterion) {
    let db = setup_db();
    let read_tx = db.begin_read().unwrap();
    let first = Transaction::first(&read_tx).unwrap().unwrap();
    let last = Transaction::last(&read_tx).unwrap().unwrap();

    c.bench_function("Transaction::all", |b| b.iter(|| Transaction::all(&read_tx).unwrap()));
    c.bench_function("Transaction::get", |b| {
        b.iter(|| Transaction::get(&read_tx, &first.id).unwrap())
    });
    c.bench_function("Transaction::get_by_hash", |b| {
        b.iter(|| Transaction::get_by_hash(&read_tx, &first.hash).unwrap())
    });
    c.bench_function("Transaction::range", |b| {
        b.iter(|| Transaction::range(&read_tx, &first.id, &last.id).unwrap())
    });
    c.bench_function("Transaction::get_utxos", |b| {
        b.iter(|| Transaction::get_utxos(&read_tx, &first.id).unwrap())
    });
}

fn benchmark_utxos(c: &mut Criterion) {
    let db = setup_db();
    let read_tx = db.begin_read().unwrap();
    let first = Utxo::first(&read_tx).unwrap().unwrap();
    let last = Utxo::last(&read_tx).unwrap().unwrap();

    c.bench_function("Utxo::all", |b| b.iter(|| Utxo::all(&read_tx).unwrap()));
    c.bench_function("Utxo::get", |b| b.iter(|| Utxo::get(&read_tx, &first.id).unwrap()));
    c.bench_function("Utxo::get_by_address", |b| {
        b.iter(|| Utxo::get_by_address(&read_tx, &first.address).unwrap())
    });
    c.bench_function("Utxo::get_by_datum", |b| {
        b.iter(|| Utxo::get_by_datum(&read_tx, &first.datum).unwrap())
    });
    c.bench_function("Utxo::range", |b| {
        b.iter(|| Utxo::range(&read_tx, &first.id, &last.id).unwrap())
    });
    c.bench_function("Utxo::get_assets", |b| {
        b.iter(|| Utxo::get_assets(&read_tx, &first.id).unwrap())
    });
}

fn benchmark_assets(c: &mut Criterion) {
    let db = setup_db();
    let read_tx = db.begin_read().unwrap();
    let first = Asset::first(&read_tx).unwrap().unwrap();
    let last = Asset::last(&read_tx).unwrap().unwrap();

    c.bench_function("Asset::all", |b| b.iter(|| Asset::all(&read_tx).unwrap()));
    c.bench_function("Asset::get", |b| b.iter(|| Asset::get(&read_tx, &first.id).unwrap()));
    c.bench_function("Asset::get_by_name", |b| {
        b.iter(|| Asset::get_by_name(&read_tx, &first.name).unwrap())
    });
    c.bench_function("Asset::get_by_policy_id", |b| {
        b.iter(|| Asset::get_by_policy_id(&read_tx, &first.policy_id).unwrap())
    });
    c.bench_function("Asset::range", |b| {
        b.iter(|| Asset::range(&read_tx, &first.id, &last.id).unwrap())
    });
}

criterion_group!(
    name = benches;
    config = configure_criterion();
    targets = benchmark_blocks, benchmark_block_headers, benchmark_transactions, benchmark_utxos, benchmark_assets
);
criterion_main!(benches);