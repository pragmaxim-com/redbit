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
        #[column(dictionary(cache = 10))]
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
model_v1::block::_store                                             270
model_v1::block::_store_and_commit                                  276
model_v1::transaction::_store                                       453
model_v1::transaction::_store_many                                  458
model_v1::transaction::_store_and_commit                            463
model_v1::block::_delete_and_commit                                 554
model_v1::block::_pk_range                                          596
model_v1::transaction::_delete_and_commit                           827
model_v1::header::_store_and_commit                                 846
model_v1::utxo::_store_and_commit                                   847
model_v1::header::_store                                            882
model_v1::header::_store_many                                       886
model_v1::utxo::_store                                              886
model_v1::transaction::_pk_range                                    896
model_v1::utxo::_store_many                                         905
model_v1::asset::_store_and_commit                                 1207
model_v1::header::_delete_and_commit                               1298
model_v1::utxo::_delete_and_commit                                 1383
model_v1::input::_store_and_commit                                 1441
model_v1::header::_pk_range                                        1499
model_v1::maybevalue::_store_and_commit                            1519
model_v1::utxo::_pk_range                                          1544
model_v1::asset::_store                                            1712
model_v1::asset::_store_many                                       1715
model_v1::asset::_delete_and_commit                                2152
model_v1::input::_store_many                                       2173
model_v1::input::_store                                            2197
model_v1::maybevalue::_store                                       2208
model_v1::maybevalue::_store_many                                  2212
model_v1::asset::_pk_range                                         2325
model_v1::input::_delete_and_commit                                2669
model_v1::maybevalue::_delete_and_commit                           2682
model_v1::maybevalue::_pk_range                                    2763
model_v1::input::_pk_range                                         2795
model_v1::block::_tail                                             4568
model_v1::block::_take                                             4573
model_v1::block::_get                                              9232
model_v1::block::_last                                             9240
model_v1::block::_first                                            9279
model_v1::block::_stream_range                                     9376
model_v1::block::_get_transactions                                 9571
model_v1::transaction::_stream_blocks_by_hash                      9777
model_v1::transaction::_stream_range                              14093
model_v1::transaction::_stream_by_hash                            14815
model_v1::transaction::_tail                                      14839
model_v1::transaction::_take                                      14962
model_v1::utxo::_stream_transactions_by_address                   15448
model_v1::transaction::_stream_ids_by_hash                        19955
model_v1::utxo::_stream_range                                     29649
model_v1::asset::_stream_utxos_by_name                            29653
model_v1::utxo::_stream_by_address                                31639
model_v1::transaction::_get_by_hash                               31645
model_v1::transaction::_first                                     32147
model_v1::transaction::_get                                       32299
model_v1::transaction::_last                                      32307
model_v1::header::_stream_range_by_duration                       37018
model_v1::header::_stream_range_by_timestamp                      37707
model_v1::utxo::_stream_ids_by_address                            38054
model_v1::header::_stream_range                                   38576
model_v1::header::_stream_by_hash                                 42274
model_v1::header::_stream_by_duration                             42352
model_v1::header::_stream_by_prev_hash                            42519
model_v1::header::_stream_by_timestamp                            42529
model_v1::header::_stream_heights_by_timestamp                    44544
model_v1::header::_stream_heights_by_duration                     44603
model_v1::header::_stream_heights_by_prev_hash                    44700
model_v1::header::_stream_heights_by_hash                         44734
model_v1::block::_range                                           48037
model_v1::block::_filter                                          53972
model_v1::transaction::_range                                     57504
model_v1::asset::_stream_range                                    60560
model_v1::transaction::_get_utxos                                 67405
model_v1::transaction::_filter                                    70986
model_v1::asset::_stream_by_name                                  71766
model_v1::asset::_stream_ids_by_name                              75615
model_v1::utxo::_tail                                             92848
model_v1::utxo::_take                                             95367
model_v1::maybevalue::_stream_range                              102733
model_v1::input::_stream_range                                   120948
model_v1::maybevalue::_stream_by_hash                            134963
model_v1::maybevalue::_stream_ids_by_hash                        139838
model_v1::utxo::_range                                           155580
model_v1::utxo::_get_by_address                                  223641
model_v1::utxo::_get                                             241966
model_v1::utxo::_first                                           243171
model_v1::utxo::_last                                            246690
model_v1::utxo::_filter                                          257927
model_v1::utxo::_get_assets                                      271561
model_v1::asset::_tail                                           300069
model_v1::asset::_range                                          312782
model_v1::asset::_take                                           344869
model_v1::header::_range_by_duration                             347360
model_v1::header::_tail                                          351603
model_v1::header::_range                                         355451
model_v1::input::_range                                          355909
model_v1::input::_tail                                           360787
model_v1::transaction::_get_inputs                               363975
model_v1::maybevalue::_range                                     368782
model_v1::header::_take                                          368892
model_v1::maybevalue::_tail                                      371303
model_v1::header::_range_by_timestamp                            377669
model_v1::input::_take                                           405439
model_v1::maybevalue::_take                                      413892
model_v1::asset::_get_by_name                                   1587428
model_v1::header::_get_by_duration                              1999960
model_v1::header::_get_by_hash                                  2314440
model_v1::header::_get_by_prev_hash                             2343237
model_v1::asset::_filter                                        2362837
model_v1::header::_get_by_timestamp                             2398714
model_v1::asset::_get                                           2554866
model_v1::header::_filter                                       2820556
model_v1::asset::_last                                          2876291
model_v1::asset::_first                                         2918345
model_v1::header::_get                                          3379634
model_v1::block::_get_header                                    3404487
model_v1::asset::_get_ids_by_name                               3408781
model_v1::utxo::_get_ids_by_address                             3529702
model_v1::header::_first                                        3552524
model_v1::header::_last                                         3585000
model_v1::maybevalue::_get_by_hash                              3828484
model_v1::header::_get_heights_by_duration                      4465881
model_v1::header::_get_heights_by_hash                          4930723
model_v1::maybevalue::_get_ids_by_hash                          5071251
model_v1::transaction::_get_ids_by_hash                         5513287
model_v1::header::_get_heights_by_prev_hash                     5552779
model_v1::header::_get_heights_by_timestamp                     5654829
model_v1::maybevalue::_get                                      6897027
model_v1::transaction::_get_maybe                               6949270
model_v1::maybevalue::_filter                                   7086670
model_v1::input::_get                                           7101768
model_v1::input::_filter                                        7126568
model_v1::maybevalue::_last                                     8716116
model_v1::input::_last                                          8775009
model_v1::maybevalue::_first                                    8879418
model_v1::input::_first                                         8898380
model_v1::asset::_exists                                       12467273
model_v1::utxo::_exists                                        15508685
model_v1::input::_exists                                       15556938
model_v1::maybevalue::_exists                                  17316017
model_v1::transaction::_exists                                 17397356
model_v1::header::_exists                                      25913449
model_v1::block::_exists                                       27517887
```
<!-- END_BENCH -->


## Chain

See [chain](./chain)
