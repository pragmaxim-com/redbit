Built for blazing fast persistence of terra bytes of structured data on a single machine
while offering rich querying capabilities, e.g. bitcoin/blockchain data. Blockchains need rich and often
analytical queries which is done through explorers because indexing speed of even embedded/in-process (not through socket) 
analytical db like [DuckDB](https://duckdb.org/) right on the node would be still slower than a hand-made solution on top of 
[Redb](https://github.com/cberner/redb) or [RocksDb](https://rocksdb.org/).

Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from
[Redb](https://github.com/cberner/redb) using secondary indexes and dictionaries, served by [axum](https://github.com/tokio-rs/axum) through auto-generated REST API.

### Major Out-of-the-Box Features

✅ Parallel persistence, there is a long-running write thread spawned for each entity field (no blocking) \
✅ Querying and ranging by secondary index \
✅ Optional dictionaries for low cardinality fields \
✅ First level DB cache (`db_cache_size_gb` is split proportionally by weights in the entity definition) :
  ```rust
  #[column(db_cache = 4)]
  #[column(index, db_cache = 4)]
  #[column(range, db_cache = 10)]
  #[column(dictionary, db_cache = 10)]
  ```
✅ LRU cache for hot indexes and dictionaries (building dictionary requires db read ) :
  ```rust
  #[column(index, lru_cache = 300_000)]
  #[column(dictionary, lru_cache = 300_000)]
  ```
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
redbit = "1.0.5"
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
        #[column(index, db_cache = 4, lru_cache = 100_000)]
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
        #[fk(one2many, db_cache = 2)]
        pub id: TransactionPointer,
        #[column]
        pub amount: u64,
        #[column(dictionary, db_cache = 10, lru_cache = 100_000)]
        pub address: Address,
        pub assets: Vec<Asset>,
    }
    
    #[entity]
    pub struct Input {
        #[fk(one2many, db_cache = 1)]
        pub id: TransactionPointer,
        #[column(db_cache = 1)]
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
        #[fk(one2many, db_cache = 1)]
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
    use redbit::storage::StorageOwner;
    
    #[tokio::main]
    async fn main() -> Result<()> {
        let (storage_owner, storage) = StorageOwner::temp("showcase", 1, true).await?;
        let blocks = Block::sample_many(2);
        let block_heights: Vec<Height> = blocks.iter().map(|b|b.height).collect();
        println!("Persisting blocks:");
        for block in blocks {
            Block::persist(Arc::clone(&storage), block)?;
        }
    
        let block_tx = Block::begin_read_ctx(&storage)?;
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
        Block::stream_range(Block::begin_read_ctx(&storage)?, first_block.height, last_block.height, None)?.try_collect::<Vec<Block>>().await?;
        Header::stream_by_hash(Header::begin_read_ctx(&storage)?, first_block_header.hash, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_by_timestamp(Header::begin_read_ctx(&storage)?, first_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_range(Header::begin_read_ctx(&storage)?, first_block_header.height, last_block_header.height, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_range_by_timestamp(Header::begin_read_ctx(&storage)?, first_block_header.timestamp, last_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
        Transaction::stream_ids_by_hash(Transaction::begin_read_ctx(&storage)?, first_transaction.hash.clone())?.try_collect::<Vec<BlockPointer>>().await?;
        Transaction::stream_by_hash(Transaction::begin_read_ctx(&storage)?, first_transaction.hash.clone(), None)?.try_collect::<Vec<Transaction>>().await?;
        Transaction::stream_range(Transaction::begin_read_ctx(&storage)?, first_transaction.id, last_transaction.id, None)?.try_collect::<Vec<Transaction>>().await?;
        Utxo::stream_ids_by_address(Utxo::begin_read_ctx(&storage)?, first_utxo.address.clone())?.try_collect::<Vec<TransactionPointer>>().await?;
        Utxo::stream_range(Utxo::begin_read_ctx(&storage)?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_address(Utxo::begin_read_ctx(&storage)?, first_utxo.address.clone(), None)?.try_collect::<Vec<Utxo>>().await?;
        // streaming parents
        Utxo::stream_transactions_by_address(Transaction::begin_read_ctx(&storage)?, first_utxo.address, None)?.try_collect::<Vec<Transaction>>().await?;
        Asset::stream_by_name(Asset::begin_read_ctx(&storage)?, first_asset.name.clone(), None)?.try_collect::<Vec<Asset>>().await?;
        Asset::stream_range(Asset::begin_read_ctx(&storage)?, first_asset.id, last_asset.id, None)?.try_collect::<Vec<Asset>>().await?;
        // streaming parents
        Asset::stream_utxos_by_name(Utxo::begin_read_ctx(&storage)?, first_asset.name, None)?.try_collect::<Vec<Utxo>>().await?;
    
        println!("
Deleting blocks:");
        for height in block_heights.into_iter() {
            Block::remove(Arc::clone(&storage), height)?;
        }
        drop(storage_owner);
        Ok(())
    }
```
<!-- END_MAIN -->

The same api is accessible through http endpoints at http://127.0.0.1:3033/swagger-ui/.

### ⏱ Redbit benchmarks (results from github servers)

The demo example persists data into 30 tables to allow for rich querying. Each `index` is backed by 2 tables and `dictionary` by 4 tables.
Each PK, FK, simple column, index or dictionary is backed by its own redb DB and a long-running indexing thread. If you have 20 of these, you are still 
fine on Raspberry Pi, consider stronger machine for deeply nested entities with many indexes and dictionaries.

Indexing process is always as slow as the column that in comparison to others has either bigger values, more values or combination of both.

See [chain](./chain) for more details on performance and data size.

The `persist` is slow because each bench iteration opens ~ 30 tables in comparison to `store` which just writes to them and commits.  
The `remove/delete` is analogous to `persist/store`

The `block::_store_many` operation in this context writes and commits 3 blocks of 3 transactions of 1 input and 3 utxos of 3 assets, ie.
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
model_v1::block::_store                                             257
model_v1::block::_store_many                                        257
model_v1::block::_persist                                           289
model_v1::transaction::_store                                       448
model_v1::transaction::_store_many                                  450
model_v1::block::_remove                                            456
model_v1::transaction::_persist                                     458
model_v1::transaction::_remove                                      680
model_v1::block::_pk_range                                          801
model_v1::utxo::_persist                                            801
model_v1::header::_persist                                          802
model_v1::header::_store_many                                       878
model_v1::header::_store                                            880
model_v1::utxo::_store_many                                         893
model_v1::utxo::_store                                              898
model_v1::transaction::_pk_range                                   1122
model_v1::header::_remove                                          1221
model_v1::utxo::_remove                                            1234
model_v1::asset::_persist                                          1307
model_v1::asset::_store_many                                       1636
model_v1::maybevalue::_persist                                     1657
model_v1::input::_persist                                          1667
model_v1::asset::_store                                            1674
model_v1::header::_pk_range                                        1699
model_v1::utxo::_pk_range                                          1700
model_v1::maybevalue::_store_many                                  1995
model_v1::input::_store                                            2011
model_v1::input::_store_many                                       2013
model_v1::asset::_remove                                           2028
model_v1::maybevalue::_store                                       2029
model_v1::asset::_pk_range                                         2315
model_v1::maybevalue::_pk_range                                    2662
model_v1::input::_pk_range                                         2699
model_v1::maybevalue::_remove                                      2989
model_v1::input::_remove                                           3012
model_v1::block::_tail                                             4632
model_v1::block::_take                                             4652
model_v1::block::_get                                              9316
model_v1::block::_last                                             9317
model_v1::block::_first                                            9321
model_v1::block::_stream_range                                     9344
model_v1::block::_get_transactions                                 9397
model_v1::transaction::_stream_blocks_by_hash                      9900
model_v1::transaction::_stream_range                              14445
model_v1::transaction::_tail                                      14603
model_v1::transaction::_take                                      14749
model_v1::utxo::_stream_transactions_by_address                   14845
model_v1::transaction::_stream_by_hash                            15046
model_v1::transaction::_stream_ids_by_hash                        19266
model_v1::utxo::_stream_range                                     28459
model_v1::transaction::_get_by_hash                               29675
model_v1::transaction::_get                                       30025
model_v1::transaction::_first                                     30088
model_v1::transaction::_last                                      30150
model_v1::asset::_stream_utxos_by_name                            30631
model_v1::utxo::_stream_by_address                                30680
model_v1::header::_stream_range_by_duration                       34161
model_v1::header::_stream_range_by_timestamp                      34494
model_v1::header::_stream_range                                   35492
model_v1::utxo::_stream_ids_by_address                            36970
model_v1::header::_stream_by_hash                                 39966
model_v1::header::_stream_heights_by_timestamp                    39999
model_v1::header::_stream_heights_by_hash                         40077
model_v1::header::_stream_by_prev_hash                            40116
model_v1::header::_stream_by_timestamp                            40180
model_v1::header::_stream_heights_by_prev_hash                    40254
model_v1::header::_stream_by_duration                             40265
model_v1::header::_stream_heights_by_duration                     41886
model_v1::block::_range                                           45082
model_v1::block::_filter                                          53438
model_v1::asset::_stream_range                                    56733
model_v1::transaction::_range                                     57530
model_v1::asset::_stream_by_name                                  64015
model_v1::transaction::_get_utxos                                 66732
model_v1::transaction::_filter                                    70188
model_v1::asset::_stream_ids_by_name                              70257
model_v1::utxo::_tail                                             92101
model_v1::utxo::_take                                             93914
model_v1::maybevalue::_stream_range                               96933
model_v1::input::_stream_range                                   110189
model_v1::maybevalue::_stream_by_hash                            124725
model_v1::maybevalue::_stream_ids_by_hash                        130384
model_v1::utxo::_range                                           153760
model_v1::utxo::_get_by_address                                  229134
model_v1::utxo::_last                                            248452
model_v1::utxo::_get                                             248711
model_v1::utxo::_first                                           249089
model_v1::utxo::_filter                                          269099
model_v1::utxo::_get_assets                                      270009
model_v1::asset::_range                                          293725
model_v1::asset::_tail                                           294290
model_v1::asset::_take                                           336197
model_v1::header::_tail                                          343000
model_v1::header::_range_by_duration                             344328
model_v1::header::_range                                         352064
model_v1::header::_take                                          356640
model_v1::maybevalue::_tail                                      360720
model_v1::maybevalue::_range                                     364016
model_v1::transaction::_get_inputs                               366717
model_v1::input::_range                                          372756
model_v1::input::_tail                                           377125
model_v1::header::_range_by_timestamp                            378579
model_v1::maybevalue::_take                                      401715
model_v1::input::_take                                           418244
model_v1::asset::_get_by_name                                   1616554
model_v1::header::_get_by_duration                              1975426
model_v1::header::_get_by_timestamp                             2337268
model_v1::header::_get_by_hash                                  2380782
model_v1::asset::_filter                                        2403384
model_v1::header::_get_by_prev_hash                             2445406
model_v1::asset::_get                                           2545306
model_v1::asset::_last                                          2936858
model_v1::header::_filter                                       2938325
model_v1::asset::_first                                         2969209
model_v1::asset::_get_ids_by_name                               3327898
model_v1::block::_get_header                                    3411572
model_v1::header::_last                                         3426183
model_v1::header::_first                                        3475118
model_v1::header::_get                                          3556567
model_v1::utxo::_get_ids_by_address                             3565444
model_v1::maybevalue::_get_by_hash                              3886816
model_v1::header::_get_heights_by_duration                      4260577
model_v1::maybevalue::_get_ids_by_hash                          4841912
model_v1::header::_get_heights_by_prev_hash                     5062778
model_v1::header::_get_heights_by_hash                          5299979
model_v1::header::_get_heights_by_timestamp                     5407451
model_v1::transaction::_get_ids_by_hash                         5449888
model_v1::transaction::_get_maybe                               6528267
model_v1::maybevalue::_filter                                   6682705
model_v1::input::_filter                                        7029877
model_v1::maybevalue::_get                                      7049203
model_v1::input::_get                                           7359435
model_v1::maybevalue::_last                                     8745846
model_v1::maybevalue::_first                                    8803592
model_v1::input::_last                                          9024456
model_v1::input::_first                                         9171788
model_v1::asset::_exists                                       11713717
model_v1::utxo::_exists                                        16289298
model_v1::input::_exists                                       16863406
model_v1::transaction::_exists                                 16863406
model_v1::maybevalue::_exists                                  17056115
model_v1::header::_exists                                      27716186
model_v1::block::_exists                                       27731559
```
<!-- END_BENCH -->


## Chain

See [chain](./chain)
