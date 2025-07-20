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

```
cd examples/utxo
cargo test       # to let all the self-generated tests run (including http layer)
cargo bench      # to run benchmarks
cargo run        # to run the demo example and start the server

cd ui
./bin/build.sh   # builds the typescript client from openapi spec
npm run test     # executes requests to all http endpoints
```

Hundreds of frontend/backend derived tests and benchmarks are executed so that if any redbit app derived from the definition compiles,
it is transparent, well tested and benched already.

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
    
    #[pointer_key(u16)] pub struct BlockPointer(Height);
    #[pointer_key(u16)] pub struct TransactionPointer(BlockPointer);
    #[pointer_key(u8)] pub struct UtxoPointer(TransactionPointer);
    
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
        pub id: BlockPointer,
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
        pub id: TransactionPointer,
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
        pub id: BlockPointer,
        #[column(index)]
        pub hash: Hash, // just dummy values
    }
    
    #[entity]
    pub struct Asset {
        #[fk(one2many)]
        pub id: UtxoPointer,
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
        Transaction::stream_ids_by_hash(&read_tx, &first_transaction.hash)?.try_collect::<Vec<BlockPointer>>().await?;
        Transaction::stream_by_hash(db.begin_read()?, first_transaction.hash.clone(), None)?.try_collect::<Vec<Transaction>>().await?;
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
        Utxo::stream_ids_by_address(&read_tx, &first_utxo.address)?.try_collect::<Vec<TransactionPointer>>().await?;
        Utxo::stream_range(db.begin_read()?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_address(db.begin_read()?, first_utxo.address.clone(), None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_datum(db.begin_read()?, first_utxo.datum.clone(), None)?.try_collect::<Vec<Utxo>>().await?;
        // even streaming parents is possible
        Utxo::stream_transactions_by_address(db.begin_read()?, first_utxo.address, None)?.try_collect::<Vec<Transaction>>().await?;
        Utxo::stream_transactions_by_datum(db.begin_read()?, first_utxo.datum, None)?.try_collect::<Vec<Transaction>>().await?;
    
        println!("Querying assets:");
        let first_asset = Asset::first(&read_tx)?.unwrap();
        let last_asset = Asset::last(&read_tx)?.unwrap();
    
        Asset::get_by_name(&read_tx, &first_asset.name)?;
        Asset::take(&read_tx, 100)?;
        Asset::get(&read_tx, &first_asset.id)?;
        Asset::range(&read_tx, &first_asset.id, &last_asset.id, None)?;
        Asset::parent_key(&read_tx, &first_asset.id)?;
        Asset::stream_by_name(db.begin_read()?, first_asset.name.clone(), None)?.try_collect::<Vec<Asset>>().await?;
        Asset::stream_range(db.begin_read()?, first_asset.id, last_asset.id, None)?.try_collect::<Vec<Asset>>().await?;
        // even streaming parents is possible
        Asset::stream_utxos_by_name(db.begin_read()?, first_asset.name, None)?.try_collect::<Vec<Utxo>>().await?;
    
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
block::_store_many                                   472
block::_store                                        940
block::_store_and_commit                             948
transaction::_store_many                            1119
block::_take                                        1450
block::_tail                                        1455
transaction::_store_and_commit                      1579
transaction::_store                                 1591
utxo::_store_many                                   2111
utxo::_store_and_commit                             2431
utxo::_store                                        2503
block::_last                                        2871
block::_get                                         2880
block::_first                                       2884
block::_get_transactions                            2929
asset::_store_many                                  3414
blockheader::_store_many                            3492
blockheader::_store                                 3520
blockheader::_store_and_commit                      3525
asset::_store                                       3689
asset::_store_and_commit                            3841
inputref::_store                                    4273
block::_delete_and_commit                           4336
inputref::_store_and_commit                         4444
inputref::_store_many                               4694
transaction::_tail                                  5030
transaction::_take                                  5057
transaction::_delete_and_commit                     5574
utxo::_delete_and_commit                            5978
blockheader::_delete_and_commit                     6321
asset::_delete_and_commit                           6359
inputref::_delete_and_commit                        6461
transaction::_last                                  9976
transaction::_get_by_hash                          10013
transaction::_get                                  10036
transaction::_first                                10065
transaction::_get_utxos                            10656
block::_range                                      17970
utxo::_tail                                        18131
block::_stream_range                               18395
utxo::_take                                        18401
block::_filter                                     19536
transaction::_stream_blocks_by_hash                21078
transaction::_range                                25187
transaction::_stream_range                         25624
transaction::_stream_by_hash                       27013
transaction::_filter                               28038
utxo::_stream_transactions_by_address              29444
utxo::_stream_transactions_by_datum                31316
utxo::_get_by_address                              32619
utxo::_get                                         34931
utxo::_get_by_datum                                34955
utxo::_first                                       35891
utxo::_last                                        37160
utxo::_range                                       38535
utxo::_stream_range                                40455
utxo::_stream_by_address                           40778
utxo::_stream_by_datum                             44152
utxo::_filter                                      45730
utxo::_get_assets                                  47647
asset::_stream_utxos_by_name                       48292
asset::_tail                                       76178
asset::_range                                      87468
asset::_take                                       92390
asset::_stream_range                              100018
asset::_stream_by_name                            117730
blockheader::_stream_range_by_mining_time         120493
asset::_get_by_name                               129268
blockheader::_tail                                132422
blockheader::_take                                132612
blockheader::_stream_range_by_timestamp           138495
asset::_filter                                    154403
asset::_get                                       154964
blockheader::_range                               169003
asset::_first                                     176696
asset::_last                                      177957
blockheader::_stream_range                        182816
inputref::_range                                  186391
blockheader::_stream_by_mining_time               194712
blockheader::_range_by_mining_time                204864
blockheader::_stream_by_hash                      206910
blockheader::_stream_by_timestamp                 210453
inputref::_stream_range                           228144
blockheader::_range_by_timestamp                  228297
block::_get_header                                228739
blockheader::_get_by_mining_time                  229594
inputref::_tail                                   233049
blockheader::_get_by_hash                         238386
blockheader::_get_by_timestamp                    244076
blockheader::_filter                              261360
blockheader::_get                                 265965
blockheader::_last                                268122
blockheader::_first                               271795
utxo::_stream_ids_by_address                      283607
asset::_pk_range                                  286386
utxo::_get_ids_by_address                         312132
asset::_stream_ids_by_name                        314189
inputref::_take                                   317312
utxo::_pk_range                                   335965
asset::_get_ids_by_name                           363486
inputref::_stream_by_hash                         395126
transaction::_pk_range                            412194
transaction::_get_input                           439549
inputref::_pk_range                               441614
inputref::_filter                                 488017
inputref::_get                                    492339
inputref::_get_by_hash                            492426
inputref::_first                                  613091
inputref::_last                                   614624
utxo::_stream_ids_by_datum                        623994
blockheader::_stream_ids_by_mining_time           657955
asset::_exists                                    693582
transaction::_stream_ids_by_hash                  711288
block::_pk_range                                  736209
utxo::_get_ids_by_datum                           777714
blockheader::_pk_range                            790152
inputref::_stream_ids_by_hash                     796045
utxo::_exists                                     826508
blockheader::_stream_ids_by_hash                  837030
blockheader::_get_ids_by_mining_time              913684
blockheader::_stream_ids_by_timestamp             915692
transaction::_get_ids_by_hash                     936733
transaction::_exists                              958240
inputref::_get_ids_by_hash                       1083741
inputref::_exists                                1090691
blockheader::_get_ids_by_hash                    1185593
blockheader::_get_ids_by_timestamp               1291506
block::_exists                                   1696698
blockheader::_exists                             1926077
```
<!-- END_BENCH -->
