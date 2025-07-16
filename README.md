Built for blazing fast persistence of terra bytes of structured data on a single machine
while offering rich querying capabilities, e.g. bitcoin/blockchain data. Blockchains need rich and often
analytical queries which is done through explorers because indexing speed of even embedded/in-process (not through socket) 
analytical db like [DuckDB](https://duckdb.org/) right on the node would be an order of magnitude slower than a hand-made solution on top of 
[Redb](https://github.com/cberner/redb) or [RocksDb](https://rocksdb.org/).

Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from
[Redb](https://github.com/cberner/redb) using secondary indexes and dictionaries, served by [axum](https://github.com/tokio-rs/axum)
through auto-generated REST API. It maximizes R/W speed while minimizing data size using hierarchical data structures of smart pointers.

### Major Out-of-the-Box Features

✅ Querying and ranging by secondary index \
✅ Optional dictionaries for low cardinality fields \
✅ One-to-One / One-to-Option / One-to-Many entities with cascade read/write/delete \
✅ All goodies including intuitive data ordering without writing custom codecs \
✅ Http response streaming api with efficient querying (ie. get txs or utxos for really HOT address) \
✅ Query contraints : `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `in` with logical `AND`
  ```json
  {
    "header": {
      "height": { "$eq": 1 }
    },
    "transactions": {
      "hash": { "$in": ["bar", "baz"] },
      "utxo": {
        "address": { "$eq": "foo" }
      }
    }
  }
  ```
✅ Column types : `String`, `Int`, `Vec<u8>`, `[u8; N]`, `bool`, `uuid::Uuid`, `chrono::DateTime`, `std::time::Duration` \
✅ Column encodings of binary columns : `hex`, `base64` \
✅ All types have binary (db) and human-readable (http) serde support \
✅ Macro derived http rest API at http://127.0.0.1:8000/swagger-ui/ with examples \
✅ Macro derived unit tests and integration tests on axum test server and benchmarks \
✅ TypeScript client generated from OpenAPI spec with tests suite requesting all endpoints 

### Limitations

❌ Root key must be newtype struct with numeric inner type (that's part of the design decision to achieve fast indexing of even whole bitcoin)


### Development

```bash
cd examples/utxo
cargo test       # to let all the self-generated tests run (including http layer)
cargo bench      # to run benchmarks
cargo run        # to run the demo example and start the server

cd ui
./bin/build.sh   # builds the typescript client from openapi spec
npm run test     # executes requests to all http endpoints
```

Hundreds of frontend/backend derived tests and benchmarks are executed.

Let's say we want to persist and query blockchain data using Redbit, declare annotated Structs `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    #![feature(test)]
    extern crate test;
    
    pub mod data;
    pub mod demo;
    pub mod routes;
    
    pub use data::*;
    pub use redbit::*;
    
    #[root_key] pub struct Height(pub u32);
    
    #[pointer_key(u16)] pub struct TxPointer(Height);
    #[pointer_key(u16)] pub struct UtxoPointer(TxPointer);
    #[pointer_key(u16)] pub struct InputPointer(TxPointer);
    #[pointer_key(u8)] pub struct AssetPointer(UtxoPointer);
    
    #[column] pub struct Hash(pub String);
    #[column("base64")] pub struct Address(pub [u8; 32]);
    #[column("hex")] pub struct Datum(pub Vec<u8>);
    #[column] pub struct AssetName(pub String);
    #[column] pub struct Time(pub chrono::DateTime<chrono::Utc>);
    #[column] pub struct Duration(pub std::time::Duration);
    #[column]
    #[derive(Copy, Hash)]
    pub struct Timestamp(pub u32);
    
    #[column]
    pub struct TempInputRef {
        tx_hash: Hash,
        index: u32,
    }
    
    #[entity]
    pub struct Block {
        #[pk]
        pub id: Height,
        pub header: BlockHeader,
        pub transactions: Vec<Transaction>,
        #[column(transient)]
        pub weight: u32,
    }
    
    #[entity]
    pub struct BlockHeader {
        #[fk(one2one)]
        pub id: Height,
        #[column(index)]
        pub hash: Hash,
        #[column(range)]
        pub timestamp: Timestamp,
        #[column(range)]
        pub mining_time: Time, // just to demonstrate a different type
        #[column]
        pub duration: Duration,
        #[column]
        pub nonce: u64,
    }
    
    #[entity]
    pub struct Transaction {
        #[fk(one2many)]
        pub id: TxPointer,
        #[column(index)]
        pub hash: Hash,
        pub utxos: Vec<Utxo>,
        pub input: Option<InputRef>, // intentionally Option to demonstrate it is possible
        #[column(transient)]
        pub transient_inputs: Vec<TempInputRef>,
    }
    
    #[entity]
    pub struct Utxo {
        #[fk(one2many)]
        pub id: UtxoPointer,
        #[column]
        pub amount: u64,
        #[column(index)]
        pub datum: Datum,
        #[column(dictionary)]
        pub address: Address,
        pub assets: Vec<Asset>,
    }
    
    #[entity]
    pub struct InputRef {
        #[fk(one2opt)]
        pub id: TxPointer,
        #[column(index)]
        pub hash: Hash, // just dummy values
    }
    
    #[entity]
    pub struct Asset {
        #[fk(one2many)]
        pub id: AssetPointer,
        #[column]
        pub amount: u64,
        #[column(dictionary)]
        pub name: AssetName,
    }
```
<!-- END_LIB -->

And R/W entire instances efficiently using indexes and dictionaries `examples/utxo/src/demo.rs`:

<!-- BEGIN_MAIN -->
```rust
    use crate::*;
    use redb::Database;
    use redbit::AppError;
    use std::sync::Arc;
    
    pub async fn run(db: Arc<Database>) -> Result<(), AppError> {
        let blocks = Block::sample_many(2);
    
        println!("Persisting blocks:");
        let write_tx = db.begin_write()?;
        Block::store_many(&write_tx, &blocks)?;
        write_tx.commit()?;
    
        let read_tx = db.begin_read()?;
    
        println!("Querying blocks:");
        let first_block = Block::first(&read_tx)?.unwrap();
        let last_block = Block::last(&read_tx)?.unwrap();
    
        Block::take(&read_tx, 100)?;
        Block::get(&read_tx, &first_block.id)?;
        Block::range(&read_tx, &first_block.id, &last_block.id, None)?;
        Block::get_transactions(&read_tx, &first_block.id)?;
        Block::get_header(&read_tx, &first_block.id)?;
        Block::exists(&read_tx, &first_block.id)?;
        Block::first(&read_tx)?;
        Block::last(&read_tx)?;
        Block::stream_range(db.begin_read()?, first_block.id, last_block.id, None)?.try_collect::<Vec<Block>>().await?;
    
        println!("Querying block headers:");
        let first_block_header = BlockHeader::first(&read_tx)?.unwrap();
        let last_block_header = BlockHeader::last(&read_tx)?.unwrap();
    
        BlockHeader::get_by_hash(&read_tx, &first_block_header.hash)?;
        BlockHeader::get_by_timestamp(&read_tx, &first_block_header.timestamp)?;
        BlockHeader::take(&read_tx, 100)?;
        BlockHeader::get(&read_tx, &first_block_header.id)?;
        BlockHeader::range(&read_tx, &first_block_header.id, &last_block_header.id, None)?;
        BlockHeader::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
        BlockHeader::stream_by_hash(db.begin_read()?, first_block_header.hash, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_by_timestamp(db.begin_read()?, first_block_header.timestamp, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_range(db.begin_read()?, first_block_header.id, last_block_header.id, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_range_by_timestamp(db.begin_read()?, first_block_header.timestamp, last_block_header.timestamp, None)?.try_collect::<Vec<BlockHeader>>().await?;
    
        println!("Querying transactions:");
        let first_transaction = Transaction::first(&read_tx)?.unwrap();
        let last_transaction = Transaction::last(&read_tx)?.unwrap();
    
        Transaction::get_ids_by_hash(&read_tx, &first_transaction.hash)?;
        Transaction::get_by_hash(&read_tx, &first_transaction.hash)?;
        Transaction::take(&read_tx, 100)?;
        Transaction::get(&read_tx, &first_transaction.id)?;
        Transaction::range(&read_tx, &first_transaction.id, &last_transaction.id, None)?;
        Transaction::get_utxos(&read_tx, &first_transaction.id)?;
        Transaction::get_input(&read_tx, &first_transaction.id)?;
        Transaction::parent_key(&read_tx, &first_transaction.id)?;
        Transaction::stream_ids_by_hash(&read_tx, &first_transaction.hash)?.try_collect::<Vec<TxPointer>>().await?;
        Transaction::stream_by_hash(db.begin_read()?, first_transaction.hash, None)?.try_collect::<Vec<Transaction>>().await?;
        Transaction::stream_range(db.begin_read()?, first_transaction.id, last_transaction.id, None)?.try_collect::<Vec<Transaction>>().await?;
    
        println!("Querying utxos:");
        let first_utxo = Utxo::first(&read_tx)?.unwrap();
        let last_utxo = Utxo::last(&read_tx)?.unwrap();
    
        Utxo::get_by_address(&read_tx, &first_utxo.address)?;
        Utxo::get_by_datum(&read_tx, &first_utxo.datum)?;
        Utxo::get_ids_by_address(&read_tx, &first_utxo.address)?;
        Utxo::take(&read_tx, 100)?;
        Utxo::get(&read_tx, &first_utxo.id)?;
        Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id, None)?;
        Utxo::get_assets(&read_tx, &first_utxo.id)?;
        Utxo::parent_key(&read_tx, &first_utxo.id)?;
        Utxo::stream_ids_by_address(&read_tx, &first_utxo.address)?.try_collect::<Vec<UtxoPointer>>().await?;
        Utxo::stream_range(db.begin_read()?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_address(db.begin_read()?, first_utxo.address, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_datum(db.begin_read()?, first_utxo.datum, None)?.try_collect::<Vec<Utxo>>().await?;
    
        println!("Querying assets:");
        let first_asset = Asset::first(&read_tx)?.unwrap();
        let last_asset = Asset::last(&read_tx)?.unwrap();
    
        Asset::get_by_name(&read_tx, &first_asset.name)?;
        Asset::take(&read_tx, 100)?;
        Asset::get(&read_tx, &first_asset.id)?;
        Asset::range(&read_tx, &first_asset.id, &last_asset.id, None)?;
        Asset::parent_key(&read_tx, &first_asset.id)?;
        Asset::stream_by_name(db.begin_read()?, first_asset.name, None)?.try_collect::<Vec<Asset>>().await?;
        Asset::stream_range(db.begin_read()?, first_asset.id, last_asset.id, None)?.try_collect::<Vec<Asset>>().await?;
    
        println!("Deleting blocks:");
        for block in blocks.iter() {
            Block::delete_and_commit(&db, &block.id)?;
        }
        Ok(())
    }
```
<!-- END_MAIN -->

The same api is accessible through http endpoints at http://127.0.0.1:8000/swagger-ui/.

Performance wise, check 🔥[flamegraph](https://rawcdn.githack.com/pragmaxim-com/redbit/refs/heads/master/flamegraph.svg).
The demo example persists data into 30 tables to allow for rich querying.

### ⏱️ Benchmark Summary
The slowest `block::_store_many` operation in this context persists 3 blocks of 3 transactions of 1 input and 3 utxos of 3 assets, ie.
the operations writes :
- 3 blocks
- 3 * 3 = 9 transactions
- 3 * 3 = 9 inputs
- 3 * 3 * 3 = 27 utxos
- 3 * 3 * 3 * 3 = 81 assets

`block::_first` operation reads whole block with all its transactions, inputs, utxos and assets.

<!-- BEGIN_BENCH -->
```
function                                           ops/s
-------------------------------------------------------------
block::_store_many                                   447
block::_store_and_commit                             991
block::_store                                        993
transaction::_store_many                            1184
block::_take                                        1534
block::_tail                                        1556
transaction::_store                                 1583
transaction::_store_and_commit                      1598
utxo::_store_many                                   2278
utxo::_store_and_commit                             2497
utxo::_store                                        2539
block::_range                                       2933
block::_stream_range                                2954
block::_filter                                      2997
block::_first                                       3017
block::_get                                         3023
block::_last                                        3044
block::_get_transactions                            3097
blockheader::_store_many                            3447
blockheader::_store                                 3468
blockheader::_store_and_commit                      3469
asset::_store_many                                  3700
asset::_store                                       3779
asset::_store_and_commit                            3890
block::_delete_and_commit                           4887
inputref::_store                                    5039
inputref::_store_many                               5229
transaction::_tail                                  5321
transaction::_take                                  5374
inputref::_store_and_commit                         5848
utxo::_delete_and_commit                            6093
transaction::_delete_and_commit                     6132
asset::_delete_and_commit                           7897
blockheader::_delete_and_commit                     8153
inputref::_delete_and_commit                        9032
transaction::_range                                10038
transaction::_stream_range                         10185
transaction::_stream_by_hash                       10405
transaction::_get_by_hash                          10424
transaction::_filter                               10458
transaction::_last                                 10502
transaction::_get                                  10572
transaction::_first                                10587
transaction::_get_utxos                            11129
utxo::_tail                                        18873
utxo::_take                                        19142
utxo::_range                                       30930
utxo::_stream_range                                32022
utxo::_stream_by_address                           32360
utxo::_get_by_address                              33386
utxo::_stream_by_datum                             34462
utxo::_filter                                      35460
utxo::_get_by_datum                                35677
utxo::_get                                         35765
utxo::_first                                       36545
utxo::_last                                        38133
utxo::_get_assets                                  48669
asset::_tail                                       79847
asset::_range                                      91319
asset::_take                                       96419
asset::_stream_range                              103560
asset::_stream_by_name                            121686
blockheader::_stream_range_by_mining_time         123348
asset::_get_by_name                               133985
blockheader::_tail                                134865
blockheader::_take                                135575
blockheader::_stream_range_by_timestamp           140372
asset::_filter                                    161469
asset::_get                                       161948
blockheader::_range                               170467
blockheader::_stream_range                        184131
asset::_first                                     185429
asset::_last                                      187070
inputref::_range                                  193122
blockheader::_stream_by_mining_time               194594
blockheader::_stream_by_hash                      210418
blockheader::_stream_by_timestamp                 211397
blockheader::_range_by_mining_time                212966
inputref::_stream_range                           234762
blockheader::_range_by_timestamp                  235770
blockheader::_get_by_mining_time                  237787
blockheader::_get_by_hash                         245313
inputref::_tail                                   246028
block::_get_header                                251767
blockheader::_get_by_timestamp                    252214
blockheader::_filter                              265353
blockheader::_get                                 271886
blockheader::_last                                273523
blockheader::_first                               274118
utxo::_stream_ids_by_address                      286584
asset::_pk_range                                  291780
asset::_stream_ids_by_name                        316229
utxo::_get_ids_by_address                         318790
inputref::_take                                   326112
utxo::_pk_range                                   346190
asset::_get_ids_by_name                           371827
inputref::_stream_by_hash                         408332
transaction::_pk_range                            432188
inputref::_pk_range                               452749
transaction::_get_input                           485312
inputref::_get_by_hash                            503292
inputref::_filter                                 505045
inputref::_get                                    508461
inputref::_first                                  623235
utxo::_stream_ids_by_datum                        624251
inputref::_last                                   629291
blockheader::_stream_ids_by_mining_time           677374
asset::_exists                                    731813
utxo::_get_ids_by_datum                           779393
inputref::_stream_ids_by_hash                     795475
blockheader::_pk_range                            805873
transaction::_stream_ids_by_hash                  820567
blockheader::_stream_ids_by_hash                  838764
utxo::_exists                                     856032
block::_pk_range                                  856737
blockheader::_stream_ids_by_timestamp             920099
blockheader::_get_ids_by_mining_time              978263
transaction::_exists                             1031183
inputref::_get_ids_by_hash                       1092526
transaction::_get_ids_by_hash                    1104484
inputref::_exists                                1149399
blockheader::_get_ids_by_hash                    1157140
blockheader::_get_ids_by_timestamp               1293494
blockheader::_exists                             1903312
block::_exists                                   2219509
```
<!-- END_BENCH -->
