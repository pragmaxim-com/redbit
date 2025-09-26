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
model_v1::block::_store                                             255
model_v1::block::_store_many                                        257
model_v1::block::_persist                                           289
model_v1::transaction::_store_many                                  447
model_v1::transaction::_store                                       452
model_v1::transaction::_persist                                     459
model_v1::block::_remove                                            461
model_v1::transaction::_remove                                      694
model_v1::utxo::_persist                                            790
model_v1::header::_persist                                          814
model_v1::block::_pk_range                                          827
model_v1::header::_store_many                                       879
model_v1::header::_store                                            880
model_v1::utxo::_store_many                                         896
model_v1::utxo::_store                                              898
model_v1::transaction::_pk_range                                   1158
model_v1::header::_remove                                          1169
model_v1::utxo::_remove                                            1222
model_v1::asset::_persist                                          1309
model_v1::maybevalue::_persist                                     1653
model_v1::asset::_store_many                                       1661
model_v1::input::_persist                                          1675
model_v1::asset::_store                                            1688
model_v1::utxo::_pk_range                                          1724
model_v1::header::_pk_range                                        1746
model_v1::maybevalue::_store                                       2041
model_v1::maybevalue::_store_many                                  2047
model_v1::input::_store_many                                       2086
model_v1::asset::_remove                                           2104
model_v1::input::_store                                            2115
model_v1::asset::_pk_range                                         2391
model_v1::maybevalue::_pk_range                                    2713
model_v1::input::_pk_range                                         2753
model_v1::maybevalue::_remove                                      3049
model_v1::input::_remove                                           3141
model_v1::block::_take                                             4430
model_v1::block::_tail                                             4523
model_v1::block::_get                                              9146
model_v1::block::_get_transactions                                 9152
model_v1::block::_stream_range                                     9156
model_v1::block::_first                                            9169
model_v1::block::_last                                             9210
model_v1::transaction::_stream_blocks_by_hash                      9875
model_v1::transaction::_stream_range                              14212
model_v1::transaction::_tail                                      14469
model_v1::transaction::_take                                      14534
model_v1::transaction::_stream_by_hash                            14775
model_v1::utxo::_stream_transactions_by_address                   14997
model_v1::transaction::_stream_ids_by_hash                        20343
model_v1::asset::_stream_utxos_by_name                            28369
model_v1::utxo::_stream_range                                     28910
model_v1::transaction::_get_by_hash                               29250
model_v1::transaction::_get                                       29394
model_v1::transaction::_first                                     29586
model_v1::transaction::_last                                      29828
model_v1::utxo::_stream_by_address                                30577
model_v1::header::_stream_range_by_duration                       36696
model_v1::utxo::_stream_ids_by_address                            36782
model_v1::header::_stream_range_by_timestamp                      37219
model_v1::header::_stream_range                                   38711
model_v1::header::_stream_by_timestamp                            41966
model_v1::header::_stream_by_duration                             42630
model_v1::header::_stream_by_hash                                 42631
model_v1::header::_stream_by_prev_hash                            42841
model_v1::header::_stream_heights_by_prev_hash                    44209
model_v1::header::_stream_heights_by_timestamp                    44332
model_v1::header::_stream_heights_by_duration                     44407
model_v1::header::_stream_heights_by_hash                         44553
model_v1::block::_range                                           45157
model_v1::block::_filter                                          53284
model_v1::transaction::_range                                     57897
model_v1::asset::_stream_range                                    58781
model_v1::transaction::_get_utxos                                 65847
model_v1::asset::_stream_by_name                                  68217
model_v1::transaction::_filter                                    68936
model_v1::asset::_stream_ids_by_name                              72674
model_v1::utxo::_tail                                             90376
model_v1::utxo::_take                                             91389
model_v1::maybevalue::_stream_range                               99197
model_v1::input::_stream_range                                   120853
model_v1::maybevalue::_stream_by_hash                            123103
model_v1::maybevalue::_stream_ids_by_hash                        136183
model_v1::utxo::_range                                           155783
model_v1::utxo::_get_by_address                                  227488
model_v1::utxo::_get                                             245941
model_v1::utxo::_last                                            246738
model_v1::utxo::_first                                           246833
model_v1::utxo::_filter                                          265358
model_v1::utxo::_get_assets                                      265796
model_v1::asset::_range                                          295334
model_v1::asset::_tail                                           297219
model_v1::header::_range_by_duration                             337610
model_v1::header::_tail                                          338658
model_v1::asset::_take                                           339037
model_v1::header::_range                                         344231
model_v1::header::_take                                          350636
model_v1::transaction::_get_inputs                               361533
model_v1::input::_tail                                           361867
model_v1::input::_range                                          363219
model_v1::header::_range_by_timestamp                            367530
model_v1::maybevalue::_range                                     368415
model_v1::maybevalue::_tail                                      372056
model_v1::maybevalue::_take                                      416696
model_v1::input::_take                                           418064
model_v1::asset::_get_by_name                                   1678359
model_v1::header::_get_by_duration                              2025398
model_v1::header::_get_by_prev_hash                             2185219
model_v1::header::_get_by_hash                                  2204343
model_v1::header::_get_by_timestamp                             2307870
model_v1::asset::_filter                                        2366696
model_v1::asset::_get                                           2555911
model_v1::header::_filter                                       2902252
model_v1::asset::_first                                         2988732
model_v1::asset::_last                                          2989269
model_v1::block::_get_header                                    3338898
model_v1::header::_get                                          3504223
model_v1::header::_last                                         3505697
model_v1::asset::_get_ids_by_name                               3543586
model_v1::header::_first                                        3612064
model_v1::utxo::_get_ids_by_address                             3679853
model_v1::maybevalue::_get_by_hash                              4040241
model_v1::header::_get_heights_by_duration                      4312947
model_v1::header::_get_heights_by_hash                          4807923
model_v1::header::_get_heights_by_prev_hash                     4870209
model_v1::transaction::_get_ids_by_hash                         5397237
model_v1::header::_get_heights_by_timestamp                     5513895
model_v1::maybevalue::_get_ids_by_hash                          5702880
model_v1::input::_filter                                        6774150
model_v1::maybevalue::_filter                                   6798097
model_v1::input::_get                                           7298737
model_v1::maybevalue::_get                                      7637086
model_v1::transaction::_get_maybe                               7741736
model_v1::maybevalue::_last                                     8237911
model_v1::input::_last                                          8801267
model_v1::maybevalue::_first                                    8820676
model_v1::input::_first                                         8937349
model_v1::asset::_exists                                       11534025
model_v1::input::_exists                                       16041065
model_v1::utxo::_exists                                        16059097
model_v1::maybevalue::_exists                                  17361111
model_v1::transaction::_exists                                 17543860
model_v1::header::_exists                                      28232637
model_v1::block::_exists                                       28433324
```
<!-- END_BENCH -->


## Chain

See [chain](./chain)
