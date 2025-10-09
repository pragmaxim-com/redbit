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

### Why redb?

Redb is B+Tree based so in comparison to LSM tree with WAL : 
  - to avoid benchmarking our SSD by random-access writes, we need to sort all data in batches before writing it
  - sorting + tree building overhead is eliminated by parallelizing writes to all columns into long-running batching threads
  - this is how we achieve both fast and predictable write performance

  - with B+tree you loose excellent sequential write performance of LSM tree, but you gain stable write performance
    - my experience is that I would need different RocksDb settings for different machines and for indexing different parts of the chain 
    => endless tuning of RocksDB parameters to really achieve that good sequential write performance

  - so B+Tree performs universally well if data is sorted upfront (not as well as with memory-mapped B+tree https://github.com/erthink/libmdbx) 
  - LSM tree can perform better if we tune the hell out of it for specific use case, environment, data, etc.

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
        #[column(index, shards = 3, db_cache = 4, lru_cache = 1)]
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
        Block::store_many(&ctx, blocks)?;
        let _ = ctx.two_phase_commit_and_close()?;
    
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

The `persist` is slower because each bench iteration opens ~ 24 tables in comparison to `store` which just writes to them and commits.  
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
model_v1::block_bench::_persist                                     158
model_v1::transaction_bench::_persist                               203
model_v1::block_bench::_remove                                      211
model_v1::block_bench::_store_many                                  228
model_v1::block_bench::_store                                       233
model_v1::transaction_bench::_remove                                252
model_v1::transaction_bench::_store_many                            356
model_v1::transaction_bench::_store                                 367
model_v1::utxo_bench::_persist                                      525
model_v1::utxo_bench::_store_many                                   549
model_v1::utxo_bench::_remove                                       575
model_v1::block_bench::_pk_range                                    586
model_v1::transaction_bench::_pk_range                              691
model_v1::utxo_bench::_store                                        788
model_v1::header_bench::_store                                      864
model_v1::header_bench::_store_many                                 871
model_v1::header_bench::_persist                                    881
model_v1::header_bench::_remove                                    1483
model_v1::asset_bench::_persist                                    1648
model_v1::utxo_bench::_pk_range                                    1796
model_v1::asset_bench::_store                                      1863
model_v1::asset_bench::_store_many                                 1873
model_v1::input_bench::_persist                                    1910
model_v1::maybevalue_bench::_persist                               1937
model_v1::asset_bench::_remove                                     2183
model_v1::maybevalue_bench::_store                                 2288
model_v1::input_bench::_remove                                     2322
model_v1::maybevalue_bench::_store_many                            2329
model_v1::maybevalue_bench::_remove                                2533
model_v1::input_bench::_store_many                                 2546
model_v1::input_bench::_store                                      2577
model_v1::header_bench::_pk_range                                  2609
model_v1::asset_bench::_pk_range                                   3055
model_v1::maybevalue_bench::_pk_range                              3373
model_v1::input_bench::_pk_range                                   3388
model_v1::block_bench::_take                                       4602
model_v1::block_bench::_tail                                       4603
model_v1::block_bench::_stream_range                               7640
model_v1::transaction_bench::_stream_blocks_by_hash                7921
model_v1::block_bench::_get                                        9593
model_v1::block_bench::_last                                       9633
model_v1::block_bench::_get_transactions                           9681
model_v1::block_bench::_first                                      9706
model_v1::transaction_bench::_stream_range                        10561
model_v1::transaction_bench::_stream_by_hash                      10914
model_v1::utxo_bench::_stream_transactions_by_address             11044
model_v1::transaction_bench::_stream_ids_by_hash                  13778
model_v1::transaction_bench::_tail                                14723
model_v1::transaction_bench::_take                                14909
model_v1::utxo_bench::_stream_range                               19468
model_v1::utxo_bench::_stream_by_address                          20568
model_v1::asset_bench::_stream_utxos_by_name                      22202
model_v1::utxo_bench::_stream_ids_by_address                      23131
model_v1::transaction_bench::_get_by_hash                         30439
model_v1::transaction_bench::_get                                 30628
model_v1::transaction_bench::_first                               30735
model_v1::transaction_bench::_last                                30929
model_v1::header_bench::_stream_range_by_duration                 36259
model_v1::header_bench::_stream_range_by_timestamp                36638
model_v1::header_bench::_stream_range                             37852
model_v1::header_bench::_stream_by_hash                           41122
model_v1::header_bench::_stream_by_timestamp                      41563
model_v1::header_bench::_stream_by_prev_hash                      41685
model_v1::header_bench::_stream_by_duration                       42022
model_v1::header_bench::_stream_heights_by_prev_hash              42914
model_v1::header_bench::_stream_heights_by_hash                   43093
model_v1::header_bench::_stream_heights_by_duration               43180
model_v1::header_bench::_stream_heights_by_timestamp              43284
model_v1::block_bench::_range                                     45609
model_v1::block_bench::_filter                                    53512
model_v1::transaction_bench::_range                               55690
model_v1::asset_bench::_stream_range                              59386
model_v1::transaction_bench::_get_utxos                           66283
model_v1::transaction_bench::_filter                              68014
model_v1::asset_bench::_stream_by_name                            69185
model_v1::asset_bench::_stream_ids_by_name                        73183
model_v1::utxo_bench::_tail                                       91122
model_v1::utxo_bench::_take                                       93962
model_v1::input_bench::_stream_range                              98541
model_v1::maybevalue_bench::_stream_range                        100992
model_v1::maybevalue_bench::_stream_by_hash                      129260
model_v1::maybevalue_bench::_stream_ids_by_hash                  137219
model_v1::utxo_bench::_range                                     146786
model_v1::utxo_bench::_get_by_address                            226666
model_v1::utxo_bench::_get                                       246877
model_v1::utxo_bench::_last                                      247506
model_v1::utxo_bench::_first                                     247689
model_v1::utxo_bench::_filter                                    263639
model_v1::utxo_bench::_get_assets                                276425
model_v1::asset_bench::_range                                    297603
model_v1::asset_bench::_tail                                     314204
model_v1::input_bench::_range                                    324599
model_v1::transaction_bench::_get_inputs                         331296
model_v1::header_bench::_tail                                    342809
model_v1::input_bench::_tail                                     349932
model_v1::asset_bench::_take                                     352005
model_v1::header_bench::_take                                    352031
model_v1::header_bench::_range_by_duration                       356448
model_v1::header_bench::_range                                   362853
model_v1::maybevalue_bench::_range                               366742
model_v1::maybevalue_bench::_tail                                381409
model_v1::header_bench::_range_by_timestamp                      385869
model_v1::input_bench::_take                                     387058
model_v1::maybevalue_bench::_take                                427592
model_v1::asset_bench::_get_by_name                             1697678
model_v1::header_bench::_get_by_duration                        2074990
model_v1::header_bench::_get_by_hash                            2384529
model_v1::header_bench::_get_by_prev_hash                       2388288
model_v1::header_bench::_get_by_timestamp                       2431966
model_v1::asset_bench::_filter                                  2608718
model_v1::asset_bench::_get                                     2745292
model_v1::header_bench::_filter                                 2832620
model_v1::asset_bench::_first                                   3151095
model_v1::asset_bench::_last                                    3158460
model_v1::block_bench::_get_header                              3515433
model_v1::asset_bench::_get_ids_by_name                         3591180
model_v1::header_bench::_get                                    3628974
model_v1::maybevalue_bench::_get_by_hash                        3674174
model_v1::header_bench::_first                                  3705351
model_v1::utxo_bench::_get_ids_by_address                       3705762
model_v1::header_bench::_last                                   3712366
model_v1::input_bench::_get                                     4447607
model_v1::input_bench::_filter                                  4499235
model_v1::header_bench::_get_heights_by_duration                4546281
model_v1::transaction_bench::_get_ids_by_hash                   4961548
model_v1::header_bench::_get_heights_by_prev_hash               5101000
model_v1::input_bench::_last                                    5126365
model_v1::input_bench::_first                                   5166624
model_v1::maybevalue_bench::_get_ids_by_hash                    5491790
model_v1::header_bench::_get_heights_by_hash                    5531585
model_v1::header_bench::_get_heights_by_timestamp               5662194
model_v1::maybevalue_bench::_filter                             7311010
model_v1::transaction_bench::_get_maybe                         7467702
model_v1::maybevalue_bench::_get                                7521625
model_v1::maybevalue_bench::_last                               9048955
model_v1::maybevalue_bench::_first                              9260117
model_v1::asset_bench::_exists                                 13034411
model_v1::input_bench::_exists                                 16772895
model_v1::utxo_bench::_exists                                  16952026
model_v1::maybevalue_bench::_exists                            17828490
model_v1::transaction_bench::_exists                           17850768
model_v1::header_bench::_exists                                27210884
model_v1::block_bench::_exists                                 27563396
```
<!-- END_BENCH -->


## Chain

See [chain](./chain)
