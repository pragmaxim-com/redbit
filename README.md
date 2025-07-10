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
✅ SSE streaming api with efficient querying (ie. get txs or utxos for really HOT address) \
✅ query contraints : `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `in` with logical `AND`
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
✅ column types : `String`, `Int`, `Vec<u8>`, `[u8; N]`, `bool`, `uuid::Uuid`, `chrono::DateTime`, `std::time::Duration` \
✅ column encodings of binary columns : `hex`, `base64` \
✅ all types have binary (db) and human-readable (http) serde support \
✅ Macro derived http rest API at http://127.0.0.1:8000/swagger-ui/ with examples \
✅ Macro derived unit tests and integration tests on axum test server

### Limitations

❌ root key must be newtype struct with numeric inner type (that's part of the design decision to achieve fast indexing of even whole bitcoin)

```
cargo run --package utxo                # to run the demo example
cargo test --package utxo               # to let all the self-generated tests run (including http layer)
```

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
    #[column] pub struct PolicyId(pub String);
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
        pub time: Time,
        #[column]
        pub duration: Duration,
        #[column(index)]
        pub merkle_root: Hash,
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
        pub tree: Option<Tree>,
    }
    
    #[entity]
    pub struct Tree {
        #[fk(one2opt)]
        pub id: UtxoPointer,
        #[column(index)]
        pub hash: Hash,
    }
    
    #[entity]
    pub struct Asset {
        #[fk(one2many)]
        pub id: AssetPointer,
        #[column]
        pub amount: u64,
        #[column(dictionary)]
        pub name: AssetName,
        #[column(dictionary)]
        pub policy_id: PolicyId,
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
        BlockHeader::get_by_merkle_root(&read_tx, &first_block_header.merkle_root)?;
        BlockHeader::take(&read_tx, 100)?;
        BlockHeader::get(&read_tx, &first_block_header.id)?;
        BlockHeader::range(&read_tx, &first_block_header.id, &last_block_header.id, None)?;
        BlockHeader::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
        BlockHeader::stream_by_hash(db.begin_read()?, first_block_header.hash, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_by_timestamp(db.begin_read()?, first_block_header.timestamp, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_by_merkle_root(db.begin_read()?, first_block_header.merkle_root, None)?.try_collect::<Vec<BlockHeader>>().await?;
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
        Utxo::get_tree(&read_tx, &first_utxo.id)?;
        Utxo::stream_ids_by_address(&read_tx, &first_utxo.address)?.try_collect::<Vec<UtxoPointer>>().await?;
        Utxo::stream_range(db.begin_read()?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_address(db.begin_read()?, first_utxo.address, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_datum(db.begin_read()?, first_utxo.datum, None)?.try_collect::<Vec<Utxo>>().await?;
    
        println!("Querying assets:");
        let first_asset = Asset::first(&read_tx)?.unwrap();
        let last_asset = Asset::last(&read_tx)?.unwrap();
    
        Asset::get_by_name(&read_tx, &first_asset.name)?;
        Asset::get_by_policy_id(&read_tx, &first_asset.policy_id)?;
        Asset::get_ids_by_policy_id(&read_tx, &first_asset.policy_id)?;
        Asset::take(&read_tx, 100)?;
        Asset::get(&read_tx, &first_asset.id)?;
        Asset::range(&read_tx, &first_asset.id, &last_asset.id, None)?;
        Asset::parent_key(&read_tx, &first_asset.id)?;
        Asset::stream_ids_by_policy_id(&read_tx, &first_asset.policy_id)?.try_collect::<Vec<AssetPointer>>().await?;
        Asset::stream_by_policy_id(db.begin_read()?, first_asset.policy_id, None)?.try_collect::<Vec<Asset>>().await?;
        Asset::stream_by_name(db.begin_read()?, first_asset.name, None)?.try_collect::<Vec<Asset>>().await?;
        Asset::stream_range(db.begin_read()?, first_asset.id, last_asset.id, None)?.try_collect::<Vec<Asset>>().await?;
    
        println!("Deleting blocks:");
        for block in blocks.iter() {
            Block::delete_and_commit(&db, &block.id)?
        }
        Ok(())
    }
```
<!-- END_MAIN -->

The same api is accessible through http endpoints at http://127.0.0.1:8000/swagger-ui/.

Performance wise, check 🔥[flamegraph](https://rawcdn.githack.com/pragmaxim-com/redbit/refs/heads/master/flamegraph.svg).
The demo example persists data into 30 tables to allow for rich querying.

### ⏱️ Benchmark Summary
The slowest `block::_store_many` operation in this context persists 3 blocks of 3 transactions of 3 utxos of 3 assets, ie.
the operations writes :
- 3 blocks
- 3 * 3 = 9 transactions
- 3 * 3 * 3 = 27 inputs
- 3 * 3 * 3 = 27 utxos
- 3 * 3 * 3 * 3 = 81 assets

`block::_first` operation reads whole block with all its transactions, inputs, utxos and assets.

<!-- BEGIN_BENCH -->
```
function                                           ops/s
-------------------------------------------------------------
block::_store                                        764
block::_store_and_commit                             770
transaction::_store_many                             978
block::_take                                        1032
transaction::_store                                 1447
transaction::_store_and_commit                      1496
utxo::_store_many                                   1937
block::_stream_range                                1965
block::_range                                       1976
block::_filter                                      1997
block::_get                                         2016
block::_first                                       2019
block::_last                                        2038
block::_get_transactions                            2055
utxo::_store                                        2394
utxo::_store_and_commit                             2428
blockheader::_store                                 2913
asset::_store                                       3036
blockheader::_store_many                            3059
asset::_store_many                                  3108
blockheader::_store_and_commit                      3245
asset::_store_and_commit                            3297
transaction::_take                                  3644
tree::_store_many                                   5784
tree::_store                                        5854
tree::_store_and_commit                             6009
block::_delete_and_commit                           6087
transaction::_range                                 6831
transaction::_stream_range                          6926
transaction::_stream_by_hash                        7025
transaction::_filter                                7044
transaction::_get_by_hash                           7122
transaction::_get                                   7139
transaction::_last                                  7151
transaction::_first                                 7161
transaction::_get_utxos                             7280
transaction::_delete_and_commit                     7563
utxo::_delete_and_commit                            7834
blockheader::_delete_and_commit                     9469
asset::_delete_and_commit                           9545
tree::_delete_and_commit                           10791
utxo::_take                                        12490
utxo::_range                                       19765
utxo::_stream_range                                21140
utxo::_stream_by_address                           21881
utxo::_get_by_address                              22434
utxo::_stream_by_datum                             22848
utxo::_filter                                      23283
utxo::_get_by_datum                                23324
utxo::_get                                         23644
utxo::_first                                       24016
utxo::_last                                        24562
utxo::_get_assets                                  31141
asset::_range                                      47709
asset::_take                                       59076
asset::_stream_range                               61490
asset::_stream_by_policy_id                        84114
asset::_stream_by_name                             85203
asset::_get_by_policy_id                           91208
asset::_get_by_name                                91530
asset::_filter                                    100844
asset::_get                                       104642
blockheader::_take                                108994
blockheader::_stream_range_by_time                111927
asset::_last                                      112853
asset::_first                                     113041
blockheader::_stream_range_by_timestamp           124073
blockheader::_range                               142829
tree::_range                                      148780
blockheader::_stream_range                        153745
blockheader::_stream_by_time                      167558
blockheader::_stream_by_hash                      175103
blockheader::_range_by_time                       175468
blockheader::_stream_by_merkle_root               176731
blockheader::_stream_by_timestamp                 180351
tree::_stream_range                               182131
block::_get_header                                186241
blockheader::_range_by_timestamp                  193170
blockheader::_get_by_time                         194669
blockheader::_filter                              204712
blockheader::_get_by_hash                         205202
blockheader::_get_by_merkle_root                  206234
blockheader::_get_by_timestamp                    207832
blockheader::_get                                 210938
blockheader::_last                                212299
blockheader::_first                               213112
tree::_take                                       265646
utxo::_stream_ids_by_address                      279943
asset::_pk_range                                  282834
utxo::_get_ids_by_address                         304195
asset::_stream_ids_by_policy_id                   312567
asset::_stream_ids_by_name                        316596
utxo::_pk_range                                   341489
tree::_stream_by_hash                             344024
tree::_pk_range                                   346737
asset::_get_ids_by_policy_id                      352568
asset::_get_ids_by_name                           354271
tree::_filter                                     377203
utxo::_get_tree                                   381990
tree::_get                                        384491
tree::_get_by_hash                                427120
transaction::_pk_range                            442247
tree::_last                                       507501
tree::_first                                      512526
utxo::_stream_ids_by_datum                        629687
blockheader::_stream_ids_by_time                  676952
asset::_exists                                    680596
blockheader::_pk_range                            727109
block::_pk_range                                  743351
transaction::_stream_ids_by_hash                  751738
utxo::_get_ids_by_datum                           769864
tree::_stream_ids_by_hash                         777708
utxo::_exists                                     822619
tree::_exists                                     831649
blockheader::_stream_ids_by_hash                  844316
blockheader::_stream_ids_by_merkle_root           861326
blockheader::_stream_ids_by_timestamp             933724
blockheader::_get_ids_by_time                     938967
tree::_get_ids_by_hash                           1000430
transaction::_get_ids_by_hash                    1016580
transaction::_exists                             1142426
blockheader::_get_ids_by_hash                    1195443
blockheader::_get_ids_by_merkle_root             1202429
blockheader::_get_ids_by_timestamp               1353345
blockheader::_exists                             1576143
block::_exists                                   1596959
```
<!-- END_BENCH -->
