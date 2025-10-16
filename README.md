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
redbit = "1.0.8"
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
        #[column(index, used, shards = 3, db_cache = 4, lru_cache = 1)]
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
        #[column(dictionary, shards = 4, db_cache = 10, lru_cache = 1)]
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
        let blocks = Block::sample_many(100);
        let block_heights: Vec<Height> = blocks.iter().map(|b|b.height).collect();
        println!("Persisting blocks:");
        let ctx = Block::begin_write_ctx(&storage)?;
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
model_v1::block_bench::_persist                                     133
model_v1::block_bench::_store_many                                  156
model_v1::block_bench::_remove                                      163
model_v1::transaction_bench::_persist                               165
model_v1::block_bench::_store                                       169
model_v1::transaction_bench::_store_many                            178
model_v1::transaction_bench::_remove                                185
model_v1::transaction_bench::_store                                 195
model_v1::utxo_bench::_persist                                      335
model_v1::utxo_bench::_remove                                       348
model_v1::utxo_bench::_store_many                                   397
model_v1::utxo_bench::_store                                        435
model_v1::header_bench::_store                                      864
model_v1::header_bench::_store_many                                 868
model_v1::header_bench::_persist                                    877
model_v1::header_bench::_remove                                    1610
model_v1::asset_bench::_store_many                                 1826
model_v1::asset_bench::_store                                      1828
model_v1::asset_bench::_persist                                    1886
model_v1::maybevalue_bench::_persist                               2319
model_v1::input_bench::_persist                                    2336
model_v1::input_bench::_store                                      2414
model_v1::input_bench::_store_many                                 2435
model_v1::maybevalue_bench::_store                                 2545
model_v1::asset_bench::_remove                                     2716
model_v1::maybevalue_bench::_store_many                            2757
model_v1::input_bench::_remove                                     2765
model_v1::maybevalue_bench::_remove                                3171
model_v1::block_bench::_take                                       4502
model_v1::block_bench::_tail                                       4530
model_v1::block_bench::_stream_range                               6740
model_v1::transaction_bench::_stream_blocks_by_hash                7004
model_v1::block_bench::_get                                        9184
model_v1::block_bench::_first                                      9186
model_v1::block_bench::_get_transactions                           9212
model_v1::block_bench::_last                                       9355
model_v1::transaction_bench::_stream_range                         9356
model_v1::transaction_bench::_stream_by_hash                       9909
model_v1::utxo_bench::_stream_transactions_by_address             11332
model_v1::transaction_bench::_stream_ids_by_hash                  12204
model_v1::transaction_bench::_tail                                14378
model_v1::transaction_bench::_take                                14626
model_v1::utxo_bench::_stream_range                               20404
model_v1::utxo_bench::_stream_by_address                          20792
model_v1::asset_bench::_stream_utxos_by_name                      22094
model_v1::utxo_bench::_stream_ids_by_address                      24205
model_v1::transaction_bench::_get_by_hash                         30214
model_v1::transaction_bench::_first                               30360
model_v1::transaction_bench::_get                                 30667
model_v1::transaction_bench::_last                                30978
model_v1::block_bench::_range                                     34869
model_v1::header_bench::_stream_range_by_timestamp                36666
model_v1::header_bench::_stream_range_by_duration                 36798
model_v1::header_bench::_stream_range                             38077
model_v1::block_bench::_filter                                    38873
model_v1::header_bench::_stream_by_duration                       40661
model_v1::header_bench::_stream_by_timestamp                      40750
model_v1::header_bench::_stream_by_hash                           40795
model_v1::header_bench::_stream_by_prev_hash                      40819
model_v1::transaction_bench::_range                               40980
model_v1::header_bench::_stream_heights_by_hash                   42288
model_v1::header_bench::_stream_heights_by_duration               43386
model_v1::header_bench::_stream_heights_by_prev_hash              43687
model_v1::header_bench::_stream_heights_by_timestamp              43830
model_v1::asset_bench::_stream_range                              59529
model_v1::transaction_bench::_get_utxos                           65492
model_v1::transaction_bench::_filter                              66456
model_v1::asset_bench::_stream_by_name                            69756
model_v1::asset_bench::_stream_ids_by_name                        73921
model_v1::utxo_bench::_tail                                       92384
model_v1::utxo_bench::_take                                       95974
model_v1::input_bench::_stream_range                              96983
model_v1::maybevalue_bench::_stream_range                         99273
model_v1::maybevalue_bench::_stream_by_hash                      128311
model_v1::maybevalue_bench::_stream_ids_by_hash                  136382
model_v1::utxo_bench::_range                                     151603
model_v1::utxo_bench::_get_by_address                            229329
model_v1::utxo_bench::_first                                     248236
model_v1::utxo_bench::_get                                       248874
model_v1::utxo_bench::_last                                      251360
model_v1::utxo_bench::_filter                                    267476
model_v1::utxo_bench::_get_assets                                282100
model_v1::asset_bench::_tail                                     302049
model_v1::asset_bench::_range                                    312019
model_v1::transaction_bench::_get_inputs                         320081
model_v1::input_bench::_tail                                     339887
model_v1::input_bench::_range                                    340934
model_v1::header_bench::_tail                                    341178
model_v1::header_bench::_range_by_duration                       341858
model_v1::asset_bench::_take                                     345833
model_v1::header_bench::_range                                   348008
model_v1::header_bench::_take                                    357779
model_v1::maybevalue_bench::_tail                                358636
model_v1::maybevalue_bench::_range                               361026
model_v1::header_bench::_range_by_timestamp                      373497
model_v1::input_bench::_take                                     381929
model_v1::maybevalue_bench::_take                                405390
model_v1::asset_bench::_get_by_name                             1595074
model_v1::header_bench::_get_by_duration                        1984245
model_v1::header_bench::_get_by_prev_hash                       2139999
model_v1::header_bench::_get_by_hash                            2153270
model_v1::header_bench::_get_by_timestamp                       2320724
model_v1::asset_bench::_filter                                  2611239
model_v1::asset_bench::_get                                     2831337
model_v1::header_bench::_filter                                 2909853
model_v1::asset_bench::_last                                    2954210
model_v1::asset_bench::_first                                   3213781
model_v1::asset_bench::_get_ids_by_name                         3364964
model_v1::block_bench::_get_header                              3431120
model_v1::header_bench::_get                                    3436899
model_v1::header_bench::_first                                  3557579
model_v1::header_bench::_last                                   3624633
model_v1::utxo_bench::_get_ids_by_address                       3668109
model_v1::maybevalue_bench::_get_by_hash                        3908540
model_v1::header_bench::_get_heights_by_duration                4348771
model_v1::input_bench::_filter                                  4518549
model_v1::input_bench::_get                                     4734400
model_v1::transaction_bench::_get_ids_by_hash                   5001751
model_v1::header_bench::_get_heights_by_prev_hash               5034486
model_v1::input_bench::_last                                    5120328
model_v1::input_bench::_first                                   5231767
model_v1::header_bench::_get_heights_by_hash                    5380394
model_v1::maybevalue_bench::_get_ids_by_hash                    5559880
model_v1::header_bench::_get_heights_by_timestamp               5900749
model_v1::maybevalue_bench::_filter                             7272198
model_v1::transaction_bench::_get_maybe                         7661661
model_v1::maybevalue_bench::_get                                7688759
model_v1::maybevalue_bench::_last                               9062075
model_v1::maybevalue_bench::_first                              9094216
model_v1::asset_bench::_exists                                 12914891
model_v1::utxo_bench::_exists                                  13296104
model_v1::maybevalue_bench::_exists                            14721036
model_v1::transaction_bench::_exists                           14744913
model_v1::input_bench::_exists                                 16152479
model_v1::header_bench::_exists                                27870680
model_v1::block_bench::_exists                                 27886224
```
<!-- END_BENCH -->


## Chain

See [chain](./chain)
