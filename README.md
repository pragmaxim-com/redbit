Built for blazing fast persistence of terra bytes of structured data on a single machine
while offering rich querying capabilities, e.g. bitcoin/blockchain data. Blockchains need rich and often
analytical queries which is done through explorers because indexing speed of even embedded/in-process (not through socket) 
analytical db like [DuckDB](https://duckdb.org/) right on the node would be still slower than a hand-made solution on top of 
[Redb](https://github.com/cberner/redb) or [RocksDb](https://rocksdb.org/).

Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from
[Redb](https://github.com/cberner/redb) using secondary indexes and dictionaries, served by [axum](https://github.com/tokio-rs/axum) through auto-generated REST API.

### Major Out-of-the-Box Features

✅ Querying and ranging by secondary index \
✅ Optional dictionaries for low cardinality fields + first level cache for building them without overhead \
✅ `One-to-One` / `One-to-Option` / `One-to-Many` entities with cascade read/write/delete \
✅ All goodies including intuitive data ordering without writing custom codecs \
✅ All keys and all newType column types with fixed-sized value implement `Copy` => minimal cloning \
✅ Http response streaming api with efficient querying (ie. get txs or utxos for really HOT address) \
✅ Query constraints : `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `in` with logical `AND`
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
✅ Column types : `String`, `Int`, `Vec<u8>`, `[u8; N]`, `bool`, `uuid::Uuid`, `std::time::Duration` \
✅ Optional column is basically `One-to-Option` relationship, we build a table for optional "values" \
✅ Column encodings of binary columns : `hex`, `base64`, `utf-8` + custom impl of `ByteVecColumnSerde` \
✅ All types have binary (db) and human-readable (http) serde support \
✅ Macro derived http rest API at http://127.0.0.1:3033/swagger-ui/ \
✅ Macro derived unit tests and integration tests on axum test server and benchmarks \
✅ TypeScript client generated from OpenAPI spec with tests suite requesting all endpoints \
✅ For other features, check the [redbit-ui](http://github.com/pragmaxim-com/redbit-ui)

### Limitations

❌ Root key must be newtype struct with numeric inner type (that's part of the design decision to achieve fast indexing of even whole bitcoin)

### Development

To use redbit in your project:

```toml
[dependencies]
redbit = "1.0.4"
```

```
cd chains/demo
cargo test                          # to let all the self-generated tests run
cargo test --features integration   # to let http layer self-generated tests run
cargo bench                         # to run benchmarks
cargo run --release                 # to run the demo example and start the server
```

Check the [redbit-ui](http://github.com/pragmaxim-com/redbit-ui) for frontend dev.

The utxo example has close to 500 frontend/backend derived tests and 130 benchmarks, so that if any redbit app derived from the definition compiles,
it is transparent, well tested and benched already.

Let's say we want to persist and query blockchain data using Redbit, declare annotated Structs `chains/demo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    pub use redbit::*;
    pub use chain::*;
    
    // feel free to add custom #[derive(Foo, Bar)] attributes to your types, they will get merged with the ones from redbit
    
    #[root_key] pub struct Height(pub u32);
    
    #[pointer_key(u16)] pub struct BlockPointer(Height);
    #[pointer_key(u16)] pub struct TransactionPointer(BlockPointer);
    #[pointer_key(u16)] pub struct UtxoPointer(TransactionPointer);
    
    // #[column] pub struct Time(pub chrono::DateTime<chrono::Utc>);
    
    #[column("hex")] pub struct BlockHash(pub [u8; 32]);
    #[column("hex")] pub struct TxHash(pub [u8; 32]);
    #[column("base64")] pub struct Address(pub Vec<u8>);
    #[column("utf-8")] pub struct AssetName(pub Vec<u8>); // String is supported but this is more efficient
    #[column] pub struct Duration(pub std::time::Duration);
    #[column] pub struct Weight(pub u32);
    
    #[column] pub struct Timestamp(pub u32);
    
    #[column]
    pub struct InputRef {
        pub tx_hash: TxHash,
        pub index: u32,
    }
    
    #[entity]
    pub struct Block {
        #[pk]
        pub height: Height,
        pub header: Header,
        pub transactions: Vec<Transaction>,
    }
    
    #[entity]
    pub struct Header {
        #[fk(one2one)]
        pub height: Height,
        #[column(index)]
        pub hash: BlockHash,
        #[column(index)]
        pub prev_hash: BlockHash,
        #[column(range)]
        pub timestamp: Timestamp,
        #[column(range)]
        pub duration: Duration,
        #[column]
        pub nonce: u64,
        #[column(transient)]
        pub weight: Weight,
    }
    
    #[entity]
    pub struct Transaction {
        #[fk(one2many)]
        pub id: BlockPointer,
        #[column(index)]
        pub hash: TxHash,
        pub utxos: Vec<Utxo>,
        #[load_from(input_refs)]
        pub inputs: Vec<Input>,
        pub maybe_value: Option<MaybeValue>, // just to demonstrate option is possible
        #[column(transient)]
        pub input_refs: Vec<InputRef>,
    }
    
    #[entity]
    pub struct Utxo {
        #[fk(one2many)]
        pub id: TransactionPointer,
        #[column]
        pub amount: u64,
        #[column(dictionary)]
        pub address: Address,
        pub assets: Vec<Asset>,
    }
    
    #[entity]
    pub struct Input {
        #[fk(one2many)]
        pub id: TransactionPointer,
    }
    
    #[entity]
    pub struct MaybeValue {
        #[fk(one2opt)]
        pub id: BlockPointer,
        #[column(index)]
        pub hash: BlockHash
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

And R/W entire instances efficiently using indexes and dictionaries `chains/demo/src/demo.rs`:

<!-- BEGIN_MAIN -->
```rust
    use anyhow::Result;
    use redbit::*;
    use std::sync::Arc;
    use demo::model_v1::*;
    
    #[tokio::main]
    async fn main() -> Result<()> {
        let storage = Storage::temp("showcase", 1, true)?;
        let blocks = Block::sample_many(2);
        let block_heights: Vec<Height> = blocks.iter().map(|b|b.height).collect();
        println!("Persisting blocks:");
        for block in blocks {
            Block::store_and_commit(Arc::clone(&storage), block)?;
        }
    
        let read_tx = storage.db.begin_read()?;
        let block_tx = Block::begin_read_tx(&read_tx)?;
        let transaction_tx = &block_tx.transactions;
        let header_tx = &block_tx.header;
        let utxo_tx = &transaction_tx.utxos;
        let maybe_value_tx = &transaction_tx.maybe_value;
        let asset_tx = &utxo_tx.assets;
    
        let first_block = Block::first(&block_tx)?.unwrap();
        let last_block = Block::last(&block_tx)?.unwrap();
    
        Block::take(&block_tx, 100)?;
        Block::get(&block_tx, &first_block.height)?;
        Block::range(&block_tx, &first_block.height, &last_block.height, None)?;
        Block::get_transactions(&transaction_tx, &first_block.height)?;
        Block::get_header(&header_tx, &first_block.height)?;
        Block::exists(&block_tx, &first_block.height)?;
        Block::first(&block_tx)?;
        Block::last(&block_tx)?;
    
        let block_infos = Block::table_info(Arc::clone(&storage))?;
        println!("Block persisted with tables :");
        for info in block_infos {
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
    
        let first_block_header = Header::first(&header_tx)?.unwrap();
        let last_block_header = Header::last(&header_tx)?.unwrap();
    
        Header::get_by_hash(&header_tx, &first_block_header.hash)?;
        Header::get_by_timestamp(&header_tx, &first_block_header.timestamp)?;
        Header::take(&header_tx, 100)?;
        Header::get(&header_tx, &first_block_header.height)?;
        Header::range(&header_tx, &first_block_header.height, &last_block_header.height, None)?;
        Header::range_by_timestamp(&header_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
    
        let block_header_infos = Header::table_info(Arc::clone(&storage))?;
        println!("
Block header persisted with tables :");
        for info in block_header_infos {
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
    
        let first_transaction = Transaction::first(&transaction_tx)?.unwrap();
        let last_transaction = Transaction::last(&transaction_tx)?.unwrap();
    
        Transaction::get_ids_by_hash(&transaction_tx, &first_transaction.hash)?;
        Transaction::get_by_hash(&transaction_tx, &first_transaction.hash)?;
        Transaction::take(&transaction_tx, 100)?;
        Transaction::get(&transaction_tx, &first_transaction.id)?;
        Transaction::range(&transaction_tx, &first_transaction.id, &last_transaction.id, None)?;
        Transaction::get_utxos(&utxo_tx, &first_transaction.id)?;
        Transaction::get_maybe_value(&maybe_value_tx, &first_transaction.id)?;
        Transaction::parent_key(&first_transaction.id)?;
    
        let transaction_infos = Transaction::table_info(Arc::clone(&storage))?;
        println!("
Transaction persisted with tables :");
        for info in transaction_infos {
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
    
        let first_utxo = Utxo::first(&utxo_tx)?.unwrap();
        let last_utxo = Utxo::last(&utxo_tx)?.unwrap();
    
        Utxo::get_by_address(&utxo_tx, first_utxo.address.clone())?;
        Utxo::get_ids_by_address(&utxo_tx, first_utxo.address.clone())?;
        Utxo::take(&utxo_tx, 100)?;
        Utxo::get(&utxo_tx, &first_utxo.id)?;
        Utxo::range(&utxo_tx, &first_utxo.id, &last_utxo.id, None)?;
        Utxo::get_assets(&asset_tx, &first_utxo.id)?;
        Utxo::parent_key(&first_utxo.id)?;
    
        let utxo_infos = Utxo::table_info(Arc::clone(&storage))?;
        println!("
Utxo persisted with tables :");
        for info in utxo_infos {
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
    
        let first_asset = Asset::first(&asset_tx)?.unwrap();
        let last_asset = Asset::last(&asset_tx)?.unwrap();
    
        Asset::get_by_name(&asset_tx, first_asset.name.clone())?;
        Asset::take(&asset_tx, 100)?;
        Asset::get(&asset_tx, &first_asset.id)?;
        Asset::range(&asset_tx, &first_asset.id, &last_asset.id, None)?;
        Asset::parent_key(&first_asset.id)?;
    
        let asset_infos = Asset::table_info(Arc::clone(&storage))?;
        println!("
Asset persisted with tables :");
        for info in asset_infos {
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
    
        /* Streaming examples */
        Block::stream_range(Block::begin_read_tx(&read_tx)?, first_block.height, last_block.height, None)?.try_collect::<Vec<Block>>().await?;
        Header::stream_by_hash(Header::begin_read_tx(&read_tx)?, first_block_header.hash, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_by_timestamp(Header::begin_read_tx(&read_tx)?, first_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_range(Header::begin_read_tx(&read_tx)?, first_block_header.height, last_block_header.height, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_range_by_timestamp(Header::begin_read_tx(&read_tx)?, first_block_header.timestamp, last_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
        Transaction::stream_ids_by_hash(Transaction::begin_read_tx(&read_tx)?, first_transaction.hash.clone())?.try_collect::<Vec<BlockPointer>>().await?;
        Transaction::stream_by_hash(Transaction::begin_read_tx(&read_tx)?, first_transaction.hash.clone(), None)?.try_collect::<Vec<Transaction>>().await?;
        Transaction::stream_range(Transaction::begin_read_tx(&read_tx)?, first_transaction.id, last_transaction.id, None)?.try_collect::<Vec<Transaction>>().await?;
        Utxo::stream_ids_by_address(Utxo::begin_read_tx(&read_tx)?, first_utxo.address.clone())?.try_collect::<Vec<TransactionPointer>>().await?;
        Utxo::stream_range(Utxo::begin_read_tx(&read_tx)?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_address(Utxo::begin_read_tx(&read_tx)?, first_utxo.address.clone(), None)?.try_collect::<Vec<Utxo>>().await?;
        // streaming parents
        Utxo::stream_transactions_by_address(Transaction::begin_read_tx(&read_tx)?, first_utxo.address, None)?.try_collect::<Vec<Transaction>>().await?;
        Asset::stream_by_name(Asset::begin_read_tx(&read_tx)?, first_asset.name.clone(), None)?.try_collect::<Vec<Asset>>().await?;
        Asset::stream_range(Asset::begin_read_tx(&read_tx)?, first_asset.id, last_asset.id, None)?.try_collect::<Vec<Asset>>().await?;
        // streaming parents
        Asset::stream_utxos_by_name(Utxo::begin_read_tx(&read_tx)?, first_asset.name, None)?.try_collect::<Vec<Utxo>>().await?;
    
        println!("
Deleting blocks:");
        for height in block_heights.iter() {
            Block::delete_and_commit(Arc::clone(&storage), height)?;
        }
        Ok(())
    }
```
<!-- END_MAIN -->

The same api is accessible through http endpoints at http://127.0.0.1:3033/swagger-ui/.

### Flamegraphs

Performance wise, check 🔥[flamegraph](https://rawcdn.githack.com/pragmaxim-com/redbit/refs/heads/master/flamegraph.svg).
The demo example persists data into 30 tables to allow for rich querying.
```
cargo flamegraph --bin target/release/demo --release
```

### ⏱ Redbit benchmarks (results from github servers)

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
function                                                          ops/s
-------------------------------------------------------------
model_v1::block::_store_and_commit                                  949
model_v1::block::_store                                             953
model_v1::block::_store_many                                        989
model_v1::transaction::_store_and_commit                           1181
model_v1::transaction::_store                                      1183
model_v1::transaction::_store_many                                 1273
model_v1::header::_store_many                                      2099
model_v1::header::_store_and_commit                                2110
model_v1::utxo::_store_and_commit                                  2126
model_v1::header::_store                                           2132
model_v1::utxo::_store                                             2135
model_v1::utxo::_store_many                                        2200
model_v1::asset::_store                                            3792
model_v1::asset::_store_many                                       3805
model_v1::asset::_store_and_commit                                 3815
model_v1::maybevalue::_store                                       4665
model_v1::maybevalue::_store_and_commit                            4738
model_v1::maybevalue::_store_many                                  4797
model_v1::input::_store_and_commit                                 6413
model_v1::input::_store                                            6450
model_v1::input::_store_many                                       6517
model_v1::block::_pk_range                                         7510
model_v1::block::_delete_and_commit                                7557
model_v1::transaction::_delete_and_commit                          8037
model_v1::transaction::_pk_range                                   8090
model_v1::header::_pk_range                                        8966
model_v1::utxo::_delete_and_commit                                 9179
model_v1::header::_delete_and_commit                               9609
model_v1::utxo::_pk_range                                          9680
model_v1::asset::_pk_range                                        10653
model_v1::maybevalue::_pk_range                                   11297
model_v1::asset::_delete_and_commit                               11504
model_v1::maybevalue::_delete_and_commit                          11709
model_v1::input::_pk_range                                        11805
model_v1::input::_delete_and_commit                               11813
model_v1::block::_take                                            19441
model_v1::block::_tail                                            19482
model_v1::block::_first                                           39768
model_v1::block::_last                                            40069
model_v1::block::_get                                             40955
model_v1::block::_get_transactions                                42218
model_v1::block::_stream_range                                    45986
model_v1::transaction::_stream_blocks_by_hash                     46715
model_v1::transaction::_tail                                      65705
model_v1::transaction::_stream_range                              65826
model_v1::transaction::_take                                      66334
model_v1::transaction::_stream_by_hash                            67759
model_v1::utxo::_stream_transactions_by_address                   72596
model_v1::transaction::_stream_ids_by_hash                       106251
model_v1::utxo::_stream_range                                    118367
model_v1::utxo::_stream_by_address                               120813
model_v1::asset::_stream_utxos_by_name                           129838
model_v1::transaction::_get_by_hash                              135814
model_v1::transaction::_get                                      138839
model_v1::transaction::_first                                    139371
model_v1::transaction::_last                                     141185
model_v1::transaction::_get_utxos                                156424
model_v1::header::_stream_range_by_duration                      157320
model_v1::header::_stream_range_by_timestamp                     161379
model_v1::utxo::_stream_ids_by_address                           169504
model_v1::block::_range                                          175975
model_v1::header::_stream_range                                  185396
model_v1::header::_stream_by_duration                            188956
model_v1::header::_stream_by_prev_hash                           189509
model_v1::header::_stream_by_timestamp                           190290
model_v1::header::_stream_by_hash                                191523
model_v1::block::_filter                                         208234
model_v1::header::_stream_heights_by_duration                    228269
model_v1::header::_stream_heights_by_prev_hash                   229913
model_v1::header::_stream_heights_by_hash                        230767
model_v1::header::_stream_heights_by_timestamp                   232706
model_v1::utxo::_tail                                            235805
model_v1::asset::_stream_range                                   244846
model_v1::utxo::_take                                            247713
model_v1::transaction::_range                                    249404
model_v1::asset::_stream_by_name                                 261525
model_v1::transaction::_filter                                   307943
model_v1::asset::_stream_ids_by_name                             325219
model_v1::maybevalue::_stream_range                              442093
model_v1::utxo::_range                                           461421
model_v1::utxo::_get_by_address                                  462423
model_v1::maybevalue::_stream_by_hash                            527574
model_v1::utxo::_first                                           529852
model_v1::utxo::_get                                             530003
model_v1::utxo::_last                                            546594
model_v1::utxo::_get_assets                                      637280
model_v1::maybevalue::_stream_ids_by_hash                        638904
model_v1::utxo::_filter                                          642546
model_v1::input::_stream_range                                   682752
model_v1::asset::_tail                                           778822
model_v1::asset::_range                                          914328
model_v1::asset::_take                                          1074322
model_v1::header::_tail                                         1177274
model_v1::header::_range_by_duration                            1214403
model_v1::header::_take                                         1275950
model_v1::header::_range                                        1297269
model_v1::maybevalue::_tail                                     1371403
model_v1::maybevalue::_range                                    1414847
model_v1::header::_range_by_timestamp                           1581753
model_v1::asset::_get_by_name                                   1589598
model_v1::input::_tail                                          1738828
model_v1::transaction::_get_inputs                              1886152
model_v1::maybevalue::_take                                     2041816
model_v1::input::_range                                         2064282
model_v1::header::_get_by_duration                              2076110
model_v1::header::_get_by_hash                                  2370904
model_v1::header::_get_by_prev_hash                             2405349
model_v1::header::_get_by_timestamp                             2448820
model_v1::asset::_filter                                        2488615
model_v1::asset::_get                                           2697308
model_v1::header::_filter                                       2994102
model_v1::asset::_first                                         3024986
model_v1::asset::_last                                          3046830
model_v1::input::_take                                          3104915
model_v1::asset::_get_ids_by_name                               3162655
model_v1::maybevalue::_get_by_hash                              3276969
model_v1::utxo::_get_ids_by_address                             3346160
model_v1::header::_get                                          3592857
model_v1::block::_get_header                                    3597640
model_v1::header::_first                                        3599453
model_v1::header::_last                                         3611673
model_v1::header::_get_heights_by_duration                      4537411
model_v1::transaction::_get_ids_by_hash                         5093984
model_v1::maybevalue::_get_ids_by_hash                          5383000
model_v1::header::_get_heights_by_hash                          5551238
model_v1::header::_get_heights_by_prev_hash                     5570410
model_v1::header::_get_heights_by_timestamp                     5690224
model_v1::maybevalue::_filter                                   7766990
model_v1::transaction::_get_maybe_value                         7773632
model_v1::maybevalue::_get                                      8008329
model_v1::maybevalue::_last                                     8899172
model_v1::maybevalue::_first                                    8947745
model_v1::asset::_exists                                       12878300
model_v1::input::_get                                          15105740
model_v1::input::_filter                                       15117158
model_v1::input::_exists                                       15549681
model_v1::utxo::_exists                                        15552100
model_v1::maybevalue::_exists                                  17593244
model_v1::transaction::_exists                                 17661604
model_v1::input::_last                                         26274304
model_v1::input::_first                                        26917900
model_v1::header::_exists                                      28058361
model_v1::block::_exists                                       28192839
```
<!-- END_BENCH -->


## Chain

[chain](./chain) syncs blockchains with nodes :
- [demo](chains/demo)
- [btc](chains/btc)
- [cardano](chains/cardano)
- [ergo](chains/ergo)

### ⏱️ Syncing performance Summary

Hand-made criterion benchmarks [deployed](https://pragmaxim-com.github.io/redbit/report/index.html).

Indexing speed in logs is the **average**, for example, the first ~ 100k **bitcoin** blocks with just one Tx have 
lower in/out indexing throughput because the block is indexed into ~ 24 tables in total.

The throughput reaches :

 - 2.0GHz & NVMe on PCIe Gen3 : `~ 17 000 Inputs+outputs / s`
 - 3.0GHz & NVMe on PCIe Gen4 : `~ 26 000 Inputs+outputs / s`
 - 4.0GHz & NVMe on PCIe Gen5 : `~ 41 000 Inputs+outputs / s`

In a nutshell, whole bitcoin up to height ~ 0.9M can be indexed in 3-4 days on a PCIe Gen5 SSD with 4.0GHz CPU.

> Note that for indexing whole bitcoin, it is better to have lots of RAM 64B+ for the Linux VM (page cache).
