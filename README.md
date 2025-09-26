Built for blazing fast persistence of terra bytes of structured data on a single machine
while offering rich querying capabilities, e.g. bitcoin/blockchain data.

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
redbit = "1.0.6"
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
model_v1::block::_store_many                                        260
model_v1::block::_persist                                           296
model_v1::block::_remove                                            450
model_v1::transaction::_store_many                                  452
model_v1::transaction::_persist                                     458
model_v1::transaction::_remove                                      682
model_v1::utxo::_persist                                            766
model_v1::header::_persist                                          800
model_v1::block::_pk_range                                          802
model_v1::header::_store_many                                       885
model_v1::utxo::_store_many                                         910
model_v1::transaction::_pk_range                                   1139
model_v1::utxo::_remove                                            1186
model_v1::header::_remove                                          1190
model_v1::asset::_persist                                          1263
model_v1::asset::_store_many                                       1615
model_v1::utxo::_pk_range                                          1618
model_v1::header::_pk_range                                        1641
model_v1::input::_persist                                          1644
model_v1::maybevalue::_persist                                     1686
model_v1::asset::_remove                                           2002
model_v1::maybevalue::_store_many                                  2075
model_v1::input::_store_many                                       2101
model_v1::asset::_pk_range                                         2225
model_v1::input::_pk_range                                         2499
model_v1::maybevalue::_pk_range                                    2744
model_v1::input::_remove                                           2907
model_v1::maybevalue::_remove                                      3017
model_v1::block::_tail                                             4520
model_v1::block::_take                                             4550
model_v1::block::_get                                              9126
model_v1::block::_first                                            9150
model_v1::block::_get_transactions                                 9229
model_v1::block::_last                                             9231
model_v1::block::_stream_range                                     9381
model_v1::transaction::_stream_blocks_by_hash                      9397
model_v1::transaction::_stream_range                              13970
model_v1::transaction::_tail                                      14046
model_v1::transaction::_take                                      14127
model_v1::transaction::_stream_by_hash                            14321
model_v1::utxo::_stream_transactions_by_address                   14787
model_v1::transaction::_stream_ids_by_hash                        19810
model_v1::block::_store                                           22844
model_v1::utxo::_stream_range                                     28945
model_v1::transaction::_get_by_hash                               29005
model_v1::transaction::_first                                     29124
model_v1::asset::_stream_utxos_by_name                            29135
model_v1::transaction::_get                                       29292
model_v1::transaction::_last                                      29342
model_v1::utxo::_stream_by_address                                30847
model_v1::header::_stream_range_by_duration                       35166
model_v1::header::_stream_range_by_timestamp                      35950
model_v1::utxo::_stream_ids_by_address                            37586
model_v1::header::_stream_range                                   37624
model_v1::header::_stream_by_prev_hash                            40700
model_v1::header::_stream_by_timestamp                            40926
model_v1::header::_stream_by_hash                                 41242
model_v1::header::_stream_heights_by_prev_hash                    41429
model_v1::header::_stream_heights_by_hash                         41800
model_v1::header::_stream_heights_by_timestamp                    42799
model_v1::header::_stream_heights_by_duration                     42990
model_v1::header::_stream_by_duration                             43075
model_v1::block::_range                                           44497
model_v1::block::_filter                                          52821
model_v1::transaction::_range                                     56893
model_v1::asset::_stream_range                                    56935
model_v1::transaction::_get_utxos                                 64287
model_v1::asset::_stream_by_name                                  67468
model_v1::transaction::_filter                                    68180
model_v1::asset::_stream_ids_by_name                              73252
model_v1::utxo::_tail                                             90762
model_v1::utxo::_take                                             92920
model_v1::maybevalue::_stream_range                               99952
model_v1::input::_stream_range                                   119384
model_v1::transaction::_store                                    122808
model_v1::maybevalue::_stream_by_hash                            132672
model_v1::maybevalue::_stream_ids_by_hash                        139064
model_v1::utxo::_range                                           150763
model_v1::utxo::_get_by_address                                  219957
model_v1::utxo::_get                                             236260
model_v1::utxo::_first                                           237317
model_v1::utxo::_last                                            239366
model_v1::utxo::_filter                                          259841
model_v1::utxo::_get_assets                                      261837
model_v1::asset::_tail                                           283735
model_v1::asset::_range                                          288774
model_v1::asset::_take                                           327750
model_v1::header::_range_by_duration                             336110
model_v1::header::_tail                                          338130
model_v1::header::_range                                         349951
model_v1::header::_take                                          349976
model_v1::maybevalue::_range                                     357992
model_v1::input::_tail                                           360049
model_v1::transaction::_get_inputs                               361555
model_v1::input::_range                                          361916
model_v1::maybevalue::_tail                                      365992
model_v1::header::_range_by_timestamp                            369050
model_v1::input::_take                                           412013
model_v1::maybevalue::_take                                      412038
model_v1::utxo::_store                                           547576
model_v1::header::_store                                        1459258
model_v1::asset::_get_by_name                                   1461305
model_v1::header::_get_by_duration                              1994336
model_v1::asset::_filter                                        2233888
model_v1::header::_get_by_prev_hash                             2275572
model_v1::header::_get_by_hash                                  2293841
model_v1::asset::_get                                           2354271
model_v1::header::_get_by_timestamp                             2414001
model_v1::asset::_last                                          2617184
model_v1::asset::_first                                         2643335
model_v1::header::_filter                                       2784042
model_v1::asset::_get_ids_by_name                               2975571
model_v1::asset::_store                                         3206259
model_v1::maybevalue::_get_by_hash                              3391440
model_v1::block::_get_header                                    3417285
model_v1::header::_get                                          3474152
model_v1::header::_first                                        3493816
model_v1::header::_last                                         3597511
model_v1::utxo::_get_ids_by_address                             3616244
model_v1::header::_get_heights_by_duration                      4474874
model_v1::maybevalue::_get_ids_by_hash                          4966723
model_v1::transaction::_get_ids_by_hash                         5287648
model_v1::header::_get_heights_by_prev_hash                     5506305
model_v1::header::_get_heights_by_hash                          5508731
model_v1::header::_get_heights_by_timestamp                     5956991
model_v1::maybevalue::_get                                      6831534
model_v1::input::_filter                                        7192692
model_v1::input::_store                                         7194245
model_v1::maybevalue::_filter                                   7276961
model_v1::transaction::_get_maybe                               7474959
model_v1::input::_get                                           7523888
model_v1::maybevalue::_store                                    7610350
model_v1::input::_last                                          8003842
model_v1::maybevalue::_last                                     8753501
model_v1::input::_first                                         8803592
model_v1::maybevalue::_first                                    8929369
model_v1::asset::_exists                                       10380982
model_v1::input::_exists                                       16949153
model_v1::maybevalue::_exists                                  17003911
model_v1::utxo::_exists                                        17006803
model_v1::transaction::_exists                                 17085255
model_v1::header::_exists                                      26716538
model_v1::block::_exists                                       27027027
```
<!-- END_BENCH -->


## Chain

See [chain](./chain)
