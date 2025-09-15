Built for blazing fast persistence of terra bytes of structured data on a single machine
while offering rich querying capabilities, e.g. bitcoin/blockchain data. Blockchains need rich and often
analytical queries which is done through explorers because indexing speed of even embedded/in-process (not through socket) 
analytical db like [DuckDB](https://duckdb.org/) right on the node would be still slower than a hand-made solution on top of 
[Redb](https://github.com/cberner/redb) or [RocksDb](https://rocksdb.org/).

Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from
[Redb](https://github.com/cberner/redb) using secondary indexes and dictionaries, served by [axum](https://github.com/tokio-rs/axum) through auto-generated REST API.

### Major Out-of-the-Box Features

✅ parallel persistence, there is a long-running write thread spawn for each entity field (no blocking) \
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
        #[write_from(input_refs)] // implement custom write_from function, see hook.rs
        pub inputs: Vec<Input>,
        pub maybe: Option<MaybeValue>, // just to demonstrate option is possible
        #[column(transient)]
        pub input_refs: Vec<InputRef>,
        #[column(transient(read_from(inputs::utxo_pointer)))] // this field is loaded when read from inputs.utxo_pointer
        pub input_utxos: Vec<Utxo>,
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
        #[column]
        pub utxo_pointer: TransactionPointer,
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
        let storage = Storage::temp("showcase", 1, true).await?;
        let blocks = Block::sample_many(2);
        let block_heights: Vec<Height> = blocks.iter().map(|b|b.height).collect();
        println!("Persisting blocks:");
        for block in blocks {
            Block::store_and_commit(Arc::clone(&storage), block)?;
        }
    
        let block_tx = Block::begin_read_tx(&storage)?;
        let transaction_tx = &block_tx.transactions;
        let header_tx = &block_tx.header;
        let utxo_tx = &transaction_tx.utxos;
        let maybe_value_tx = &transaction_tx.maybe;
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
    
        let block_infos = Block::table_info(&storage)?;
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
    
        let block_header_infos = Header::table_info(&storage)?;
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
        Transaction::get_maybe(&maybe_value_tx, &first_transaction.id)?;
        Transaction::parent_key(&first_transaction.id)?;
    
        let transaction_infos = Transaction::table_info(&storage)?;
        println!("
Transaction persisted with tables :");
        for info in transaction_infos {
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
    
        let first_utxo = Utxo::first(&utxo_tx)?.unwrap();
        let last_utxo = Utxo::last(&utxo_tx)?.unwrap();
    
        Utxo::get_by_address(&utxo_tx, &first_utxo.address)?;
        Utxo::get_ids_by_address(&utxo_tx, &first_utxo.address)?;
        Utxo::take(&utxo_tx, 100)?;
        Utxo::get(&utxo_tx, &first_utxo.id)?;
        Utxo::range(&utxo_tx, &first_utxo.id, &last_utxo.id, None)?;
        Utxo::get_assets(&asset_tx, &first_utxo.id)?;
        Utxo::parent_key(&first_utxo.id)?;
    
        let utxo_infos = Utxo::table_info(&storage)?;
        println!("
Utxo persisted with tables :");
        for info in utxo_infos {
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
    
        let first_asset = Asset::first(&asset_tx)?.unwrap();
        let last_asset = Asset::last(&asset_tx)?.unwrap();
    
        Asset::get_by_name(&asset_tx, &first_asset.name)?;
        Asset::take(&asset_tx, 100)?;
        Asset::get(&asset_tx, &first_asset.id)?;
        Asset::range(&asset_tx, &first_asset.id, &last_asset.id, None)?;
        Asset::parent_key(&first_asset.id)?;
    
        let asset_infos = Asset::table_info(&storage)?;
        println!("
Asset persisted with tables :");
        for info in asset_infos {
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
    
        /* Streaming examples */
        Block::stream_range(Block::begin_read_tx(&storage)?, first_block.height, last_block.height, None)?.try_collect::<Vec<Block>>().await?;
        Header::stream_by_hash(Header::begin_read_tx(&storage)?, first_block_header.hash, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_by_timestamp(Header::begin_read_tx(&storage)?, first_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_range(Header::begin_read_tx(&storage)?, first_block_header.height, last_block_header.height, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_range_by_timestamp(Header::begin_read_tx(&storage)?, first_block_header.timestamp, last_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
        Transaction::stream_ids_by_hash(Transaction::begin_read_tx(&storage)?, first_transaction.hash.clone())?.try_collect::<Vec<BlockPointer>>().await?;
        Transaction::stream_by_hash(Transaction::begin_read_tx(&storage)?, first_transaction.hash.clone(), None)?.try_collect::<Vec<Transaction>>().await?;
        Transaction::stream_range(Transaction::begin_read_tx(&storage)?, first_transaction.id, last_transaction.id, None)?.try_collect::<Vec<Transaction>>().await?;
        Utxo::stream_ids_by_address(Utxo::begin_read_tx(&storage)?, first_utxo.address.clone())?.try_collect::<Vec<TransactionPointer>>().await?;
        Utxo::stream_range(Utxo::begin_read_tx(&storage)?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_address(Utxo::begin_read_tx(&storage)?, first_utxo.address.clone(), None)?.try_collect::<Vec<Utxo>>().await?;
        // streaming parents
        Utxo::stream_transactions_by_address(Transaction::begin_read_tx(&storage)?, first_utxo.address, None)?.try_collect::<Vec<Transaction>>().await?;
        Asset::stream_by_name(Asset::begin_read_tx(&storage)?, first_asset.name.clone(), None)?.try_collect::<Vec<Asset>>().await?;
        Asset::stream_range(Asset::begin_read_tx(&storage)?, first_asset.id, last_asset.id, None)?.try_collect::<Vec<Asset>>().await?;
        // streaming parents
        Asset::stream_utxos_by_name(Utxo::begin_read_tx(&storage)?, first_asset.name, None)?.try_collect::<Vec<Utxo>>().await?;
    
        println!("
Deleting blocks:");
        for height in block_heights.into_iter() {
            Block::delete_and_commit(Arc::clone(&storage), height)?;
        }
        Ok(())
    }
```
<!-- END_MAIN -->

The same api is accessible through http endpoints at http://127.0.0.1:3033/swagger-ui/.

### ⏱ Redbit benchmarks (results from github servers)

The demo example persists data into 30 tables to allow for rich querying. Each `index` is backed by 2 tables and `dictionary` by 4 tables.
Each simple column, index or dictionary is backed by its own redb DB and a long-running indexing thread. If you have 20 of these, you are still 
fine on Raspberry Pi, consider stronger machine for deeply nested entities with many indexes and dictionaries.

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
model_v1::block::_store_many                                        269
model_v1::block::_store_and_commit                                  409
model_v1::block::_store                                             413
model_v1::transaction::_store_many                                  573
model_v1::transaction::_store                                       702
model_v1::transaction::_store_and_commit                            750
model_v1::block::_delete_and_commit                                 849
model_v1::block::_pk_range                                          880
model_v1::header::_store_many                                       895
model_v1::header::_store_and_commit                                 947
model_v1::header::_store                                            954
model_v1::utxo::_store_many                                         984
model_v1::utxo::_store_and_commit                                  1037
model_v1::utxo::_store                                             1073
model_v1::header::_delete_and_commit                               1230
model_v1::transaction::_pk_range                                   1269
model_v1::asset::_store                                            1319
model_v1::asset::_store_many                                       1349
model_v1::header::_pk_range                                        1372
model_v1::transaction::_delete_and_commit                          1416
model_v1::maybevalue::_store                                       1417
model_v1::asset::_store_and_commit                                 1445
model_v1::maybevalue::_store_many                                  1457
model_v1::maybevalue::_store_and_commit                            1465
model_v1::utxo::_delete_and_commit                                 1623
model_v1::utxo::_pk_range                                          1722
model_v1::asset::_delete_and_commit                                1862
model_v1::asset::_pk_range                                         1883
model_v1::maybevalue::_delete_and_commit                           1994
model_v1::maybevalue::_pk_range                                    2149
model_v1::input::_store_and_commit                                 2880
model_v1::input::_store_many                                       3045
model_v1::input::_store                                            3341
model_v1::block::_take                                             4526
model_v1::block::_tail                                             4587
model_v1::input::_delete_and_commit                                5055
model_v1::input::_pk_range                                         5424
model_v1::block::_get_transactions                                 9384
model_v1::block::_get                                              9385
model_v1::block::_first                                            9423
model_v1::block::_last                                             9424
model_v1::block::_stream_range                                    10104
model_v1::transaction::_stream_blocks_by_hash                     10425
model_v1::transaction::_tail                                      14595
model_v1::transaction::_take                                      14688
model_v1::transaction::_stream_range                              15357
model_v1::transaction::_stream_by_hash                            15955
model_v1::utxo::_stream_transactions_by_address                   16452
model_v1::transaction::_stream_ids_by_hash                        22709
model_v1::header::_stream_by_duration                             23659
model_v1::header::_stream_range_by_duration                       26410
model_v1::header::_stream_range_by_timestamp                      26465
model_v1::header::_stream_range                                   27326
model_v1::transaction::_get_by_hash                               30539
model_v1::transaction::_get                                       30891
model_v1::transaction::_first                                     30962
model_v1::transaction::_last                                      31110
model_v1::utxo::_stream_range                                     31897
model_v1::asset::_stream_utxos_by_name                            34002
model_v1::utxo::_stream_by_address                                34338
model_v1::header::_stream_by_timestamp                            35461
model_v1::header::_stream_by_hash                                 35517
model_v1::header::_stream_by_prev_hash                            35530
model_v1::header::_stream_heights_by_duration                     36520
model_v1::header::_stream_heights_by_timestamp                    36636
model_v1::header::_stream_heights_by_prev_hash                    36639
model_v1::header::_stream_heights_by_hash                         36765
model_v1::utxo::_stream_ids_by_address                            41512
model_v1::block::_range                                           46525
model_v1::block::_filter                                          53402
model_v1::transaction::_range                                     56620
model_v1::asset::_stream_range                                    63519
model_v1::transaction::_get_utxos                                 64490
model_v1::transaction::_filter                                    68859
model_v1::asset::_stream_by_name                                  75831
model_v1::asset::_stream_ids_by_name                              82823
model_v1::utxo::_tail                                             91843
model_v1::utxo::_take                                             94670
model_v1::maybevalue::_stream_range                              100572
model_v1::maybevalue::_stream_by_hash                            130449
model_v1::maybevalue::_stream_ids_by_hash                        135729
model_v1::input::_stream_range                                   136360
model_v1::utxo::_range                                           151730
model_v1::utxo::_get_by_address                                  230476
model_v1::header::_tail                                          232571
model_v1::header::_range                                         239106
model_v1::header::_take                                          240224
model_v1::input::_range                                          242888
model_v1::header::_range_by_timestamp                            244548
model_v1::utxo::_first                                           250408
model_v1::utxo::_get                                             251131
model_v1::utxo::_last                                            252384
model_v1::utxo::_filter                                          272405
model_v1::utxo::_get_assets                                      279190
model_v1::asset::_tail                                           297454
model_v1::asset::_range                                          312196
model_v1::header::_range_by_duration                             326258
model_v1::asset::_take                                           334324
model_v1::transaction::_get_inputs                               349715
model_v1::input::_tail                                           355766
model_v1::maybevalue::_range                                     364953
model_v1::maybevalue::_tail                                      368927
model_v1::maybevalue::_take                                      411387
model_v1::input::_take                                           412419
model_v1::asset::_get_by_name                                   1497634
model_v1::header::_get_by_duration                              2062323
model_v1::header::_get_by_prev_hash                             2221186
model_v1::asset::_filter                                        2305157
model_v1::header::_get_by_timestamp                             2329428
model_v1::header::_get_by_hash                                  2346481
model_v1::asset::_get                                           2487438
model_v1::asset::_last                                          2692080
model_v1::asset::_first                                         2739126
model_v1::header::_filter                                       2800571
model_v1::block::_get_header                                    3254785
model_v1::header::_get                                          3366777
model_v1::header::_first                                        3392591
model_v1::utxo::_get_ids_by_address                             3497971
model_v1::asset::_get_ids_by_name                               3536193
model_v1::maybevalue::_get_by_hash                              3546225
model_v1::header::_last                                         3592986
model_v1::header::_get_heights_by_duration                      4372349
model_v1::maybevalue::_get_ids_by_hash                          4765990
model_v1::header::_get_heights_by_hash                          5417705
model_v1::header::_get_heights_by_prev_hash                     5424759
model_v1::transaction::_get_ids_by_hash                         5489679
model_v1::header::_get_heights_by_timestamp                     5574136
model_v1::input::_filter                                        6642753
model_v1::maybevalue::_get                                      6699270
model_v1::transaction::_get_maybe                               6908463
model_v1::input::_get                                           7077642
model_v1::maybevalue::_filter                                   7172058
model_v1::maybevalue::_last                                     8314625
model_v1::maybevalue::_first                                    8625151
model_v1::input::_last                                          8697921
model_v1::input::_first                                         8920607
model_v1::asset::_exists                                       11717835
model_v1::maybevalue::_exists                                  14564521
model_v1::input::_exists                                       15976993
model_v1::utxo::_exists                                        16023073
model_v1::transaction::_exists                                 17277125
model_v1::header::_exists                                      27631943
model_v1::block::_exists                                       27639580
```
<!-- END_BENCH -->


## Chain

See [chain](./chain)
