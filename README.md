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
    use crate::block_chain::BlockChain;
    
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
    pub struct TempInputRef {
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
        pub inputs: Vec<Input>,
        pub maybe_value: Option<MaybeValue>, // just to demonstrate option is possible
        #[column(transient)]
        pub temp_input_refs: Vec<TempInputRef>,
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
    
        Utxo::get_by_address(&utxo_tx, &first_utxo.address)?;
        Utxo::get_ids_by_address(&utxo_tx, &first_utxo.address)?;
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
    
        Asset::get_by_name(&asset_tx, &first_asset.name)?;
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
model_v1::block::_store                                             962
model_v1::block::_store_and_commit                                  969
model_v1::block::_store_many                                       1007
model_v1::transaction::_store_many                                 1372
model_v1::transaction::_store_and_commit                           1483
model_v1::transaction::_store                                      1502
model_v1::utxo::_store_and_commit                                  2321
model_v1::header::_store                                           2354
model_v1::utxo::_store                                             2364
model_v1::header::_store_many                                      2374
model_v1::header::_store_and_commit                                2377
model_v1::utxo::_store_many                                        2441
model_v1::asset::_store_and_commit                                 3704
model_v1::asset::_store                                            3739
model_v1::asset::_store_many                                       3767
model_v1::maybevalue::_store_and_commit                            3899
model_v1::input::_store                                            4561
model_v1::maybevalue::_store                                       4603
model_v1::input::_store_many                                       4766
model_v1::maybevalue::_store_many                                  4846
model_v1::input::_store_and_commit                                 4950
model_v1::block::_pk_range                                         5047
model_v1::transaction::_delete_and_commit                          5269
model_v1::transaction::_pk_range                                   5522
model_v1::block::_delete_and_commit                                5614
model_v1::header::_delete_and_commit                               5650
model_v1::maybevalue::_pk_range                                    5684
model_v1::input::_delete_and_commit                                6115
model_v1::utxo::_pk_range                                          6125
model_v1::header::_pk_range                                        6173
model_v1::asset::_delete_and_commit                                6340
model_v1::utxo::_delete_and_commit                                 6905
model_v1::asset::_pk_range                                         6937
model_v1::input::_pk_range                                         7066
model_v1::maybevalue::_delete_and_commit                           9871
model_v1::block::_take                                            17804
model_v1::block::_tail                                            18276
model_v1::block::_get_transactions                                36591
model_v1::block::_first                                           36703
model_v1::block::_get                                             36774
model_v1::block::_last                                            37299
model_v1::block::_stream_range                                    42758
model_v1::transaction::_stream_blocks_by_hash                     45332
model_v1::transaction::_tail                                      57772
model_v1::transaction::_take                                      59718
model_v1::transaction::_stream_range                              64961
model_v1::transaction::_stream_by_hash                            67134
model_v1::utxo::_stream_transactions_by_address                   70311
model_v1::transaction::_stream_ids_by_hash                       104691
model_v1::utxo::_stream_range                                    114773
model_v1::utxo::_stream_by_address                               118464
model_v1::transaction::_get_by_hash                              121393
model_v1::transaction::_get                                      123571
model_v1::asset::_stream_utxos_by_name                           124690
model_v1::transaction::_first                                    125930
model_v1::transaction::_last                                     128604
model_v1::transaction::_get_utxos                                144544
model_v1::header::_stream_range_by_duration                      155322
model_v1::block::_range                                          156715
model_v1::header::_stream_range_by_timestamp                     161317
model_v1::utxo::_stream_ids_by_address                           169089
model_v1::header::_stream_range                                  180901
model_v1::header::_stream_by_duration                            187073
model_v1::header::_stream_by_prev_hash                           189202
model_v1::header::_stream_by_hash                                189242
model_v1::block::_filter                                         190298
model_v1::header::_stream_by_timestamp                           191057
model_v1::utxo::_tail                                            207937
model_v1::transaction::_range                                    219459
model_v1::header::_stream_heights_by_prev_hash                   221755
model_v1::header::_stream_heights_by_duration                    222477
model_v1::utxo::_take                                            224648
model_v1::header::_stream_heights_by_hash                        224916
model_v1::header::_stream_heights_by_timestamp                   225671
model_v1::asset::_stream_range                                   236775
model_v1::asset::_stream_by_name                                 261150
model_v1::transaction::_filter                                   279267
model_v1::asset::_stream_ids_by_name                             324781
model_v1::utxo::_range                                           409060
model_v1::maybevalue::_stream_range                              422470
model_v1::utxo::_get_by_address                                  438297
model_v1::utxo::_get                                             492701
model_v1::utxo::_first                                           493717
model_v1::maybevalue::_stream_by_hash                            500746
model_v1::utxo::_last                                            509261
model_v1::maybevalue::_stream_ids_by_hash                        576389
model_v1::utxo::_get_assets                                      584949
model_v1::utxo::_filter                                          595976
model_v1::input::_stream_range                                   663183
model_v1::asset::_tail                                           709683
model_v1::asset::_range                                          832355
model_v1::asset::_take                                           986310
model_v1::header::_tail                                         1003039
model_v1::header::_take                                         1079634
model_v1::header::_range                                        1098660
model_v1::header::_range_by_duration                            1123066
model_v1::maybevalue::_range                                    1160120
model_v1::maybevalue::_tail                                     1226678
model_v1::header::_range_by_timestamp                           1474578
model_v1::input::_tail                                          1565239
model_v1::asset::_get_by_name                                   1620063
model_v1::transaction::_get_inputs                              1686227
model_v1::maybevalue::_take                                     1738496
model_v1::input::_range                                         1763451
model_v1::header::_get_by_duration                              1959171
model_v1::header::_get_by_timestamp                             2216656
model_v1::header::_get_by_hash                                  2259070
model_v1::header::_get_by_prev_hash                             2317121
model_v1::input::_take                                          2375748
model_v1::asset::_filter                                        2416218
model_v1::asset::_get                                           2576124
model_v1::asset::_last                                          2813494
model_v1::asset::_first                                         2845598
model_v1::header::_filter                                       2935564
model_v1::block::_get_header                                    3244857
model_v1::header::_last                                         3248757
model_v1::header::_get                                          3311368
model_v1::header::_first                                        3423368
model_v1::asset::_get_ids_by_name                               3545596
model_v1::utxo::_get_ids_by_address                             3669321
model_v1::maybevalue::_get_by_hash                              3915273
model_v1::header::_get_heights_by_duration                      4356160
model_v1::header::_get_heights_by_hash                          4400440
model_v1::maybevalue::_get_ids_by_hash                          4921987
model_v1::transaction::_get_ids_by_hash                         5325948
model_v1::header::_get_heights_by_prev_hash                     5473454
model_v1::header::_get_heights_by_timestamp                     5639522
model_v1::maybevalue::_filter                                   7196833
model_v1::maybevalue::_get                                      7273785
model_v1::transaction::_get_maybe_value                         7287036
model_v1::maybevalue::_last                                     8389966
model_v1::maybevalue::_first                                    8552857
model_v1::asset::_exists                                       12088975
model_v1::maybevalue::_exists                                  13912076
model_v1::input::_get                                          15144631
model_v1::transaction::_exists                                 15506280
model_v1::input::_filter                                       15583606
model_v1::input::_exists                                       15966789
model_v1::utxo::_exists                                        16007684
model_v1::input::_last                                         24236549
model_v1::input::_first                                        25297243
model_v1::header::_exists                                      27019724
model_v1::block::_exists                                       27188690
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

If node and indexer each uses its own SSD, then the throughput reaches :

 - 2.0GHz & NVMe on PCIe Gen3 : `~ 17 000 Inputs+outputs / s`
 - 3.0GHz & NVMe on PCIe Gen4 : `~ 26 000 Inputs+outputs / s`
 - 4.0GHz & NVMe on PCIe Gen5 : `~ 41 000 Inputs+outputs / s`

In a nutshell, whole bitcoin up to height ~ 0.9M can be indexed in 3-4 days on a PCIe Gen5 SSD with 4.0GHz CPU.
