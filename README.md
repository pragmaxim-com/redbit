Built for blazing fast persistence of terra bytes of structured data on a single machine
while offering rich querying capabilities, e.g. bitcoin/blockchain data.

Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from
[Redb](https://github.com/cberner/redb) using secondary indexes and dictionaries, served by [axum](https://github.com/tokio-rs/axum) through auto-generated REST API.

Databases are data volume/quantity agnostic, it is up to developer to index and query data reasonably.
Redbit is designed with this in mind and developer sets # of shards, db cache and lru cache for HOT columns to
make them catch up with others even if they are HOT, see [chain](chain/README.md) to see how it performs on blockchain data.

### Major Out-of-the-Box Features

✅ Parallel persistence, there is a long-running write thread spawned for each entity column (no blocking) \
✅ Querying and ranging by secondary index \
✅ Optional dictionaries for low cardinality fields or for building unique values (addresses) \
✅ Sharding of columns which parallelizes their indexing (high quantity/volume columns) \
  ```rust
  #[column(shards = 4)]
  #[column(index, shards = 4)]
  #[column(dictionary, shards = 4)]
  ```
✅ First level DB cache (`db_cache_size_gb` is split proportionally by weights in the entity definition) :
  ```rust
  #[column(db_cache = 4)]
  #[column(index, db_cache = 4)]
  #[column(range, db_cache = 10)]
  #[column(dictionary, db_cache = 10)]
  ```
✅ LRU cache for hot indexes and dictionaries (building dictionary requires a db read) :
  ```rust
  #[column(index, lru_cache = 3)]
  #[column(dictionary, lru_cache = 3)]
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

### Why and when redb?

Redb is copy-on-write (COW) B+Tree based so in comparison to LSM tree with WAL or memory-mapped COW like LMDB, in order 
to avoid benchmarking our SSD by random-access writes, ie. to rapidly reduce write amplification, we need to : 

    - systematically combine durable and non-durable writes to leverage Linux VM (page cache) and reduce amount of fsync calls
    - sort all data in batches before writing it to reduce tree building overhead
      - solved by parallelizing writes to all columns into long-running batching threads

### Development

To use redbit in your project:

```toml
[dependencies]
redbit = "1.0.9"
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
        #[column(index, used, shards = 3, db_cache = 4)]
        pub hash: TxHash,
        pub utxos: Vec<Utxo>,
        #[write_from_using(input_refs, hash)] // implement custom write_from_using function, see hook.rs
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
        #[column(shards = 3)]
        pub amount: u64,
        #[column(dictionary, shards = 4, db_cache = 10)]
        pub address: Address,
        pub assets: Vec<Asset>,
    }
    
    #[entity]
    pub struct Input {
        #[fk(one2many, db_cache = 1)]
        pub id: TransactionPointer,
        #[column(db_cache = 1, shards = 2)]
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
    use redbit::storage::init::StorageOwner;
    
    #[tokio::main]
    async fn main() -> Result<()> {
        let (storage_owner, storage) = StorageOwner::temp("showcase", 1, true).await?;
        let blocks = Block::sample_many(3);
        let block_heights: Vec<Height> = blocks.iter().map(|b|b.height).collect();
        println!("Persisting blocks:");
        let ctx = Block::begin_write_ctx(&storage, Durability::None)?;
        Block::store_many(&ctx, blocks, true)?;
        let _ = ctx.two_phase_commit_and_close(MutationType::Writes)?;
    
        let block_read_ctx = Block::begin_read_ctx(&storage)?;
        
        let first_block = Block::first(&block_read_ctx)?.unwrap();
        let last_block = Block::last(&block_read_ctx)?.unwrap();
    
        Block::take(&block_read_ctx, 100)?;
        Block::get(&block_read_ctx, first_block.height)?;
        Block::range(&block_read_ctx, first_block.height, last_block.height, None)?;
        Block::exists(&block_read_ctx, first_block.height)?;
        Block::first(&block_read_ctx)?;
        Block::last(&block_read_ctx)?;
    
        let tx_read_ctx = &block_read_ctx.transactions;
        let header_read_ctx = &block_read_ctx.header;
        Block::get_transactions(tx_read_ctx, first_block.height)?;
        Block::get_header(header_read_ctx, first_block.height)?;
    
        Block::table_info(&storage)?;
    
        let first_block_header = Header::first(header_read_ctx)?.unwrap();
        let last_block_header = Header::last(header_read_ctx)?.unwrap();
    
        Header::get_by_hash(header_read_ctx, &first_block_header.hash)?;
        Header::get_by_timestamp(header_read_ctx, &first_block_header.timestamp)?;
        Header::take(header_read_ctx, 100)?;
        Header::get(header_read_ctx, first_block_header.height)?;
        Header::range(header_read_ctx, first_block_header.height, last_block_header.height, None)?;
        Header::range_by_timestamp(header_read_ctx, &first_block_header.timestamp, &last_block_header.timestamp)?;
    
        let first_transaction = Transaction::first(tx_read_ctx)?.unwrap();
        let last_transaction = Transaction::last(tx_read_ctx)?.unwrap();
    
        Transaction::get_ids_by_hash(tx_read_ctx, &first_transaction.hash)?;
        Transaction::get_by_hash(tx_read_ctx, &first_transaction.hash)?;
        Transaction::take(tx_read_ctx, 100)?;
        Transaction::get(tx_read_ctx, first_transaction.id)?;
        Transaction::range(tx_read_ctx, first_transaction.id, last_transaction.id, None)?;
        Transaction::parent_key(first_transaction.id)?;
    
        let utxo_read_ctx = &tx_read_ctx.utxos;
        let maybe_value_read_ctx = &tx_read_ctx.maybe;
    
        Transaction::get_utxos(utxo_read_ctx, first_transaction.id)?;
        Transaction::get_maybe(maybe_value_read_ctx, first_transaction.id)?;
    
        let first_utxo = Utxo::first(utxo_read_ctx)?.unwrap();
        let last_utxo = Utxo::last(utxo_read_ctx)?.unwrap();
    
        Utxo::get_by_address(utxo_read_ctx, &first_utxo.address)?;
        Utxo::get_ids_by_address(utxo_read_ctx, &first_utxo.address)?;
        Utxo::take(utxo_read_ctx, 100)?;
        Utxo::get(utxo_read_ctx, first_utxo.id)?;
        Utxo::range(utxo_read_ctx, first_utxo.id, last_utxo.id, None)?;
        Utxo::parent_key(first_utxo.id)?;
    
        let asset_read_ctx = &utxo_read_ctx.assets;
        Utxo::get_assets(asset_read_ctx, first_utxo.id)?;
    
        let first_asset = Asset::first(asset_read_ctx)?.unwrap();
        let last_asset = Asset::last(asset_read_ctx)?.unwrap();
    
        Asset::get_by_name(asset_read_ctx, &first_asset.name)?;
        Asset::take(asset_read_ctx, 100)?;
        Asset::get(asset_read_ctx, first_asset.id)?;
        Asset::range(asset_read_ctx, first_asset.id, last_asset.id, None)?;
        Asset::parent_key(first_asset.id)?;
    
        /* Streaming examples */
        Block::stream_range(Block::begin_read_ctx(&storage)?, first_block.height, last_block.height, None)?.try_collect::<Vec<Block>>().await?;
        Header::stream_by_hash(Header::begin_read_ctx(&storage)?, first_block_header.hash, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_by_timestamp(Header::begin_read_ctx(&storage)?, first_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_range(Header::begin_read_ctx(&storage)?, first_block_header.height, last_block_header.height, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_range_by_timestamp(Header::begin_read_ctx(&storage)?, first_block_header.timestamp, last_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
        Transaction::stream_ids_by_hash(Transaction::begin_read_ctx(&storage)?, first_transaction.hash)?.try_collect::<Vec<BlockPointer>>().await?;
        Transaction::stream_by_hash(Transaction::begin_read_ctx(&storage)?, first_transaction.hash, None)?.try_collect::<Vec<Transaction>>().await?;
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

The demo example persists data into 24 tables to allow for rich querying. Each `index` is backed by 2 tables and `dictionary` by 4 tables.
Each PK, FK, simple column, index or dictionary is backed by its own redb DB and a long-running indexing thread. If you have 20 of these, you are still 
fine on Raspberry Pi, consider stronger machine for deeply nested entities with many indexes and dictionaries.

Indexing process is always as slow as the column which in comparison to others has either bigger values, more values or combination of both.

See [chain](./chain) for more details on performance and data size.

The `persist` is slower because each bench iteration opens ~ 24 databases with tables in comparison to `store` which just writes to them and commits.  
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
model_v1::block_bench::_persist                                     221
model_v1::transaction_bench::_store                                 266
model_v1::utxo_bench::_store                                        266
model_v1::utxo_bench::_store_many                                   277
model_v1::input_bench::_store                                       353
model_v1::block_bench::_store                                       374
model_v1::transaction_bench::_persist                               375
model_v1::block_bench::_remove                                      380
model_v1::block_bench::_store_many                                  388
model_v1::transaction_bench::_store_many                            414
model_v1::input_bench::_store_many                                  442
model_v1::transaction_bench::_remove                                570
model_v1::utxo_bench::_persist                                      785
model_v1::header_bench::_persist                                    888
model_v1::utxo_bench::_remove                                       969
model_v1::header_bench::_remove                                    1521
model_v1::asset_bench::_persist                                    1917
model_v1::asset_bench::_remove                                     2333
model_v1::input_bench::_persist                                    2361
model_v1::maybevalue_bench::_persist                               2517
model_v1::input_bench::_remove                                     2693
model_v1::maybevalue_bench::_remove                                3165
model_v1::header_bench::_store_many                                3892
model_v1::header_bench::_store                                     4187
model_v1::block_bench::_take                                       4506
model_v1::block_bench::_tail                                       4542
model_v1::asset_bench::_store_many                                 6479
model_v1::block_bench::_stream_range                               6576
model_v1::transaction_bench::_stream_blocks_by_hash                6605
model_v1::asset_bench::_store                                      7086
model_v1::maybevalue_bench::_store_many                            8553
model_v1::transaction_bench::_stream_range                         9039
model_v1::block_bench::_first                                      9106
model_v1::block_bench::_get                                        9136
model_v1::block_bench::_get_transactions                           9182
model_v1::block_bench::_last                                       9352
model_v1::transaction_bench::_stream_by_hash                       9428
model_v1::maybevalue_bench::_store                                 9569
model_v1::utxo_bench::_stream_transactions_by_address             10937
model_v1::transaction_bench::_stream_ids_by_hash                  11840
model_v1::transaction_bench::_take                                13636
model_v1::transaction_bench::_tail                                14112
model_v1::utxo_bench::_stream_range                               19590
model_v1::utxo_bench::_stream_by_address                          20500
model_v1::asset_bench::_stream_utxos_by_name                      21432
model_v1::utxo_bench::_stream_ids_by_address                      23111
model_v1::transaction_bench::_get_by_hash                         28992
model_v1::transaction_bench::_last                                29377
model_v1::transaction_bench::_first                               29944
model_v1::transaction_bench::_get                                 30031
model_v1::block_bench::_range                                     34280
model_v1::header_bench::_stream_range_by_duration                 35564
model_v1::header_bench::_stream_range_by_timestamp                35702
model_v1::header_bench::_stream_range                             36721
model_v1::block_bench::_filter                                    39121
model_v1::header_bench::_stream_by_duration                       39840
model_v1::header_bench::_stream_by_timestamp                      40058
model_v1::header_bench::_stream_by_prev_hash                      40174
model_v1::transaction_bench::_range                               40186
model_v1::header_bench::_stream_by_hash                           40218
model_v1::header_bench::_stream_heights_by_duration               42157
model_v1::header_bench::_stream_heights_by_hash                   42678
model_v1::header_bench::_stream_heights_by_timestamp              42719
model_v1::header_bench::_stream_heights_by_prev_hash              42768
model_v1::asset_bench::_stream_range                              58698
model_v1::transaction_bench::_get_utxos                           64226
model_v1::transaction_bench::_filter                              64441
model_v1::asset_bench::_stream_by_name                            67752
model_v1::asset_bench::_stream_ids_by_name                        71115
model_v1::utxo_bench::_tail                                       91550
model_v1::maybevalue_bench::_stream_range                         93921
model_v1::utxo_bench::_take                                       94576
model_v1::input_bench::_stream_range                              96444
model_v1::maybevalue_bench::_stream_by_hash                      121955
model_v1::maybevalue_bench::_stream_ids_by_hash                  130590
model_v1::utxo_bench::_range                                     146471
model_v1::utxo_bench::_get_by_address                            226273
model_v1::utxo_bench::_get                                       243412
model_v1::utxo_bench::_first                                     244648
model_v1::utxo_bench::_last                                      246350
model_v1::utxo_bench::_get_assets                                251727
model_v1::utxo_bench::_filter                                    266351
model_v1::asset_bench::_tail                                     282890
model_v1::asset_bench::_range                                    311174
model_v1::asset_bench::_take                                     316322
model_v1::header_bench::_tail                                    330198
model_v1::transaction_bench::_get_inputs                         330288
model_v1::header_bench::_range                                   333387
model_v1::input_bench::_range                                    333975
model_v1::header_bench::_take                                    341802
model_v1::header_bench::_range_by_duration                       343186
model_v1::input_bench::_tail                                     345774
model_v1::maybevalue_bench::_tail                                362236
model_v1::header_bench::_range_by_timestamp                      366948
model_v1::maybevalue_bench::_range                               367003
model_v1::input_bench::_take                                     392094
model_v1::maybevalue_bench::_take                                406754
model_v1::asset_bench::_get_by_name                             1702881
model_v1::header_bench::_get_by_duration                        2113137
model_v1::header_bench::_get_by_hash                            2280814
model_v1::header_bench::_get_by_timestamp                       2364625
model_v1::header_bench::_get_by_prev_hash                       2370511
model_v1::asset_bench::_filter                                  2478438
model_v1::asset_bench::_get                                     2820318
model_v1::header_bench::_filter                                 2898551
model_v1::asset_bench::_last                                    2945508
model_v1::asset_bench::_first                                   2973182
model_v1::utxo_bench::_get_ids_by_address                       3234466
model_v1::block_bench::_get_header                              3509018
model_v1::header_bench::_get                                    3592599
model_v1::header_bench::_last                                   3599712
model_v1::asset_bench::_get_ids_by_name                         3642456
model_v1::header_bench::_first                                  3656708
model_v1::maybevalue_bench::_get_by_hash                        3843936
model_v1::header_bench::_get_heights_by_duration                4202210
model_v1::input_bench::_filter                                  4576659
model_v1::transaction_bench::_get_ids_by_hash                   4701457
model_v1::input_bench::_get                                     4803997
model_v1::input_bench::_first                                   5164489
model_v1::input_bench::_last                                    5202914
model_v1::header_bench::_get_heights_by_prev_hash               5216484
model_v1::header_bench::_get_heights_by_timestamp               5411548
model_v1::maybevalue_bench::_get_ids_by_hash                    5429766
model_v1::header_bench::_get_heights_by_hash                    5558644
model_v1::maybevalue_bench::_filter                             7797271
model_v1::maybevalue_bench::_get                                7909515
model_v1::transaction_bench::_get_maybe                         8192692
model_v1::maybevalue_bench::_last                               9225092
model_v1::maybevalue_bench::_first                              9229349
model_v1::asset_bench::_exists                                 13048017
model_v1::input_bench::_exists                                 17044486
model_v1::utxo_bench::_exists                                  17379214
model_v1::transaction_bench::_exists                           17815785
model_v1::maybevalue_bench::_exists                            18047284
model_v1::header_bench::_exists                                27901786
model_v1::block_bench::_exists                                 28169014
```
<!-- END_BENCH -->


## Chain

See [chain](./chain)
