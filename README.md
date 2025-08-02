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
✅ `One-to-One` / `One-to-Option` / `One-to-Many` entities with cascade read/write/delete \
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
✅ Optional column is basically `One-to-Option` relationship, we build a table for optional "values" \
✅ Column encodings of binary columns : `hex`, `base64`, `utf-8`, `btc_base58`, `btc_bech32`, `btc_addr`, `cardano_base58`, `cardano_bech32`, `cardano_addr` \
✅ All types have binary (db) and human-readable (http) serde support \
✅ Macro derived http rest API at http://127.0.0.1:8000/swagger-ui/ with examples \
✅ Macro derived unit tests and integration tests on axum test server and benchmarks \
✅ TypeScript client generated from OpenAPI spec with tests suite requesting all endpoints \
✅ For other features, check the [redbit-ui](http://github.com/pragmaxim-com/redbit-ui)

### Limitations

❌ Root key must be newtype struct with numeric inner type (that's part of the design decision to achieve fast indexing of even whole bitcoin)


### Development

```
cd examples/utxo
cargo test       # to let all the self-generated tests run (including http layer)
cargo bench      # to run benchmarks
cargo run        # to run the demo example and start the server
```

Check the [redbit-ui](http://github.com/pragmaxim-com/redbit-ui) for frontend dev.

The utxo example has close to 500 frontend/backend derived tests and 130 benchmarks, so that if any redbit app derived from the definition compiles,
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
    
    #[column("hex")] pub struct Hash(pub [u8; 32]);
    #[column("btc_addr")] pub struct BtcAddress(pub Vec<u8>);
    #[column("cardano_addr")] pub struct CardanoAddress(pub Vec<u8>);
    #[column("utf-8")] pub struct AssetName(pub Vec<u8>); // String is supported but this is more efficient
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
        pub height: Height,
        pub header: BlockHeader,
        pub transactions: Vec<Transaction>,
        #[column(transient)]
        pub weight: u32,
    }
    
    #[entity]
    pub struct BlockHeader {
        #[fk(one2one)]
        pub height: Height,
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
        #[column(dictionary)]
        pub btc_address: BtcAddress,
        #[column(dictionary)]
        pub cardano_address: CardanoAddress,
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
        Block::get(&read_tx, &first_block.height)?;
        Block::range(&read_tx, &first_block.height, &last_block.height, None)?;
        Block::get_transactions(&read_tx, &first_block.height)?;
        Block::get_header(&read_tx, &first_block.height)?;
        Block::exists(&read_tx, &first_block.height)?;
        Block::first(&read_tx)?;
        Block::last(&read_tx)?;
        Block::stream_range(db.begin_read()?, first_block.height, last_block.height, None)?.try_collect::<Vec<Block>>().await?;
    
        println!("Querying block headers:");
        let first_block_header = BlockHeader::first(&read_tx)?.unwrap();
        let last_block_header = BlockHeader::last(&read_tx)?.unwrap();
    
        BlockHeader::get_by_hash(&read_tx, &first_block_header.hash)?;
        BlockHeader::get_by_timestamp(&read_tx, &first_block_header.timestamp)?;
        BlockHeader::take(&read_tx, 100)?;
        BlockHeader::get(&read_tx, &first_block_header.height)?;
        BlockHeader::range(&read_tx, &first_block_header.height, &last_block_header.height, None)?;
        BlockHeader::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
        BlockHeader::stream_by_hash(db.begin_read()?, first_block_header.hash, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_by_timestamp(db.begin_read()?, first_block_header.timestamp, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_range(db.begin_read()?, first_block_header.height, last_block_header.height, None)?.try_collect::<Vec<BlockHeader>>().await?;
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
    
        Utxo::get_by_btc_address(&read_tx, &first_utxo.btc_address)?;
        Utxo::get_ids_by_btc_address(&read_tx, &first_utxo.btc_address)?;
        Utxo::take(&read_tx, 100)?;
        Utxo::get(&read_tx, &first_utxo.id)?;
        Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id, None)?;
        Utxo::get_assets(&read_tx, &first_utxo.id)?;
        Utxo::parent_key(&read_tx, &first_utxo.id)?;
        Utxo::stream_ids_by_btc_address(&read_tx, &first_utxo.btc_address)?.try_collect::<Vec<TransactionPointer>>().await?;
        Utxo::stream_range(db.begin_read()?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_btc_address(db.begin_read()?, first_utxo.btc_address.clone(), None)?.try_collect::<Vec<Utxo>>().await?;
        // even streaming parents is possible
        Utxo::stream_transactions_by_btc_address(db.begin_read()?, first_utxo.btc_address, None)?.try_collect::<Vec<Transaction>>().await?;
    
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
            Block::delete_and_commit(&db, &block.height)?;
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
block::_store_many                                   423
block::_store_and_commit                             856
block::_store                                        861
transaction::_store_many                            1049
block::_tail                                        1371
block::_take                                        1373
transaction::_store                                 1627
transaction::_store_and_commit                      1628
utxo::_store_many                                   1968
utxo::_store                                        2321
utxo::_store_and_commit                             2357
block::_get                                         2731
block::_first                                       2745
block::_last                                        2748
block::_get_transactions                            2800
blockheader::_store_many                            3144
blockheader::_store                                 3387
blockheader::_store_and_commit                      3389
asset::_store_many                                  3450
asset::_store                                       3675
asset::_store_and_commit                            3704
inputref::_store_many                               4100
inputref::_store                                    4262
inputref::_store_and_commit                         4274
transaction::_take                                  4722
transaction::_tail                                  4730
block::_delete_and_commit                           4813
transaction::_delete_and_commit                     5139
utxo::_delete_and_commit                            5344
inputref::_delete_and_commit                        6052
asset::_delete_and_commit                           6081
blockheader::_delete_and_commit                     7255
transaction::_get_by_hash                           9213
transaction::_get                                   9328
transaction::_first                                 9373
transaction::_last                                  9402
transaction::_get_utxos                             9972
block::_range                                      16565
block::_stream_range                               17024
utxo::_tail                                        17200
utxo::_take                                        17363
block::_filter                                     18152
transaction::_stream_blocks_by_hash                18878
transaction::_range                                23227
transaction::_stream_range                         24006
transaction::_stream_by_hash                       24376
utxo::_stream_transactions_by_btc_address          25617
utxo::_stream_transactions_by_cardano_address      25683
transaction::_filter                               25814
utxo::_get_by_btc_address                          28455
utxo::_get_by_cardano_address                      28567
utxo::_get                                         32656
utxo::_first                                       33250
utxo::_stream_by_btc_address                       34135
utxo::_last                                        34604
utxo::_stream_by_cardano_address                   34628
utxo::_range                                       35815
utxo::_stream_range                                37255
utxo::_filter                                      41652
asset::_stream_utxos_by_name                       43603
utxo::_get_assets                                  48442
asset::_tail                                       78199
asset::_range                                      89416
asset::_take                                       94991
asset::_stream_range                              101047
blockheader::_stream_range_by_mining_time         110660
asset::_stream_by_name                            119776
blockheader::_take                                121961
blockheader::_tail                                123141
blockheader::_stream_range_by_timestamp           125685
asset::_get_by_name                               128960
blockheader::_stream_by_hash                      149848
blockheader::_range                               152650
asset::_filter                                    157203
asset::_get                                       159341
utxo::_stream_ids_by_btc_address                  161071
inputref::_range                                  163695
blockheader::_stream_range                        164617
utxo::_stream_ids_by_cardano_address              169217
blockheader::_get_by_hash                         169510
utxo::_get_ids_by_btc_address                     169731
asset::_first                                     179551
utxo::_get_ids_by_cardano_address                 181728
asset::_last                                      181936
blockheader::_stream_by_mining_time               185066
blockheader::_range_by_mining_time                193455
inputref::_stream_range                           202839
blockheader::_stream_by_timestamp                 203511
inputref::_tail                                   210411
block::_get_header                                214321
blockheader::_range_by_timestamp                  215755
blockheader::_get_by_mining_time                  215779
inputref::_stream_by_hash                         232022
blockheader::_get_by_timestamp                    234255
blockheader::_filter                              244836
blockheader::_get                                 247466
blockheader::_last                                248272
blockheader::_first                               250350
inputref::_get_by_hash                            262041
inputref::_take                                   273302
asset::_pk_range                                  283851
asset::_stream_ids_by_name                        310176
transaction::_stream_ids_by_hash                  324912
utxo::_pk_range                                   342720
asset::_get_ids_by_name                           359908
inputref::_stream_ids_by_hash                     360181
blockheader::_stream_heights_by_hash              370328
transaction::_get_ids_by_hash                     383046
transaction::_get_input                           387180
blockheader::_get_heights_by_hash                 401824
inputref::_get_ids_by_hash                        412089
transaction::_pk_range                            424277
inputref::_get                                    436258
inputref::_filter                                 438618
inputref::_pk_range                               454225
inputref::_last                                   527301
inputref::_first                                  529243
blockheader::_stream_heights_by_mining_time       650229
asset::_exists                                    714582
block::_pk_range                                  728513
blockheader::_pk_range                            767790
utxo::_exists                                     847443
blockheader::_stream_heights_by_timestamp         914788
blockheader::_get_heights_by_mining_time          925523
transaction::_exists                              977670
inputref::_exists                                1131439
blockheader::_get_heights_by_timestamp           1311802
block::_exists                                   1636045
blockheader::_exists                             1918024
```
<!-- END_BENCH -->
