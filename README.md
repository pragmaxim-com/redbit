Built for blazing fast persistence of terra bytes of structured data on a single machine
while offering rich querying capabilities, e.g. bitcoin/blockchain data. Blockchains need rich and often
analytical queries which is done through explorers because indexing speed of even embedded/in-process (not through socket) 
analytical db like [DuckDB](https://duckdb.org/) right on the node would be still slower than a hand-made solution on top of 
[Redb](https://github.com/cberner/redb) or [RocksDb](https://rocksdb.org/).

Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from
[Redb](https://github.com/cberner/redb) using secondary indexes and dictionaries, served by [axum](https://github.com/tokio-rs/axum)
through auto-generated REST API.

### Major Out-of-the-Box Features

✅ Querying and ranging by secondary index \
✅ Optional dictionaries for low cardinality fields + first level cache for building them without overhead \
✅ `One-to-One` / `One-to-Option` / `One-to-Many` entities with cascade read/write/delete \
✅ All goodies including intuitive data ordering without writing custom codecs \
✅ All keys and all newType column types with fixed-sized value implement `Copy` => minimal cloning \
✅ Http response streaming api with efficient querying (ie. get txs or utxos for really HOT address) \
✅ Query contraints : `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `in` with logical `AND`
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
✅ Macro derived http rest API at http://127.0.0.1:3033/swagger-ui/ with examples \
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
cd examples/utxo
cargo test                          # to let all the self-generated tests run
cargo test --features integration   # to let http layer self-generated tests run
cargo bench                         # to run benchmarks
cargo run --release                 # to run the demo example and start the server
```

Check the [redbit-ui](http://github.com/pragmaxim-com/redbit-ui) for frontend dev.

The utxo example has close to 500 frontend/backend derived tests and 130 benchmarks, so that if any redbit app derived from the definition compiles,
it is transparent, well tested and benched already.

Let's say we want to persist and query blockchain data using Redbit, declare annotated Structs `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    pub use redbit::*;
    
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
        pub inputs: Vec<InputRef>,
        pub maybe_value: Option<MaybeValue>, // just to demonstrate option is possible
        #[column(transient)]
        pub transient_inputs: Vec<TempInputRef>,
    }
    
    #[entity]
    pub struct Utxo {
        #[fk(one2many)]
        pub id: TransactionPointer,
        #[column]
        pub amount: u64,
        #[column(dictionary(cache = 10000))]
        pub address: Address,
        pub assets: Vec<Asset>,
    }
    
    #[entity]
    pub struct InputRef {
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
        #[column(dictionary(cache = 10000))]
        pub name: AssetName,
    }
    
    use chain::api::*;
    
    pub struct BlockChain {
        pub storage: Arc<Storage>,
    }
    
    impl BlockChain {
        pub fn new(storage: Arc<Storage>) -> Arc<dyn BlockChainLike<Block>> {
            Arc::new(BlockChain { storage })
        }
    
        fn resolve_tx_inputs(&self, read_tx: &StorageReadTx, block: &mut Block) -> Result<(), ChainError> {
            for tx in &mut block.transactions {
                for transient_input in tx.transient_inputs.iter_mut() {
                    let tx_pointers = Transaction::get_ids_by_hash(read_tx, &transient_input.tx_hash)?;
    
                    match tx_pointers.first() {
                        Some(tx_pointer) => tx.inputs.push(InputRef { id: TransactionPointer::from_parent(*tx_pointer, transient_input.index as u16) }),
                        None => tx.inputs.push(InputRef { id: TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0) }),
                    }
                }
            }
            Ok(())
        }
    }
```
<!-- END_LIB -->

And R/W entire instances efficiently using indexes and dictionaries `examples/utxo/src/demo.rs`:

<!-- BEGIN_MAIN -->
```rust
    use redbit::{AppError, Storage};
    use std::sync::Arc;
    use crate::model_v1::*;
    
    pub async fn showcase() -> Result<(), AppError> {
        let storage = Storage::temp("showcase", 1, true)?;
        let blocks = Block::sample_many(2);
        println!("Persisting blocks:");
        let write_tx = storage.begin_write()?;
        Block::store_many(&write_tx, &blocks)?;
        write_tx.commit()?;
    
        let read_tx = storage.begin_read()?;
    
        let first_block = Block::first(&read_tx)?.unwrap();
        let last_block = Block::last(&read_tx)?.unwrap();
    
        Block::take(&read_tx, 100)?;
        Block::get(&read_tx, &first_block.height)?;
        Block::range(&read_tx, &first_block.height, &last_block.height, None)?;
        Block::get_transactions(&read_tx, &first_block.height)?;
        Block::get_header(&read_tx, &first_block.height)?;
        Block::exists(&read_tx, &first_block.height)?;
        Block::first(&read_tx)?;
        Block::last(&read_tx)?;
        Block::stream_range(storage.begin_read()?, first_block.height, last_block.height, None)?.try_collect::<Vec<Block>>().await?;
    
        let block_infos = Block::table_info(Arc::clone(&storage))?;
        println!("Block persisted with tables :");
        for info in block_infos {
            println!("{}", serde_json::to_string_pretty(&info).unwrap());
        }
    
        let first_block_header = Header::first(&read_tx)?.unwrap();
        let last_block_header = Header::last(&read_tx)?.unwrap();
    
        Header::get_by_hash(&read_tx, &first_block_header.hash)?;
        Header::get_by_timestamp(&read_tx, &first_block_header.timestamp)?;
        Header::take(&read_tx, 100)?;
        Header::get(&read_tx, &first_block_header.height)?;
        Header::range(&read_tx, &first_block_header.height, &last_block_header.height, None)?;
        Header::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
        Header::stream_by_hash(storage.begin_read()?, first_block_header.hash, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_by_timestamp(storage.begin_read()?, first_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_range(storage.begin_read()?, first_block_header.height, last_block_header.height, None)?.try_collect::<Vec<Header>>().await?;
        Header::stream_range_by_timestamp(storage.begin_read()?, first_block_header.timestamp, last_block_header.timestamp, None)?.try_collect::<Vec<Header>>().await?;
    
        let block_header_infos = Header::table_info(Arc::clone(&storage))?;
        println!("
Block header persisted with tables :");
        for info in block_header_infos {
            println!("{}", serde_json::to_string_pretty(&info).unwrap());
        }
    
        let first_transaction = Transaction::first(&read_tx)?.unwrap();
        let last_transaction = Transaction::last(&read_tx)?.unwrap();
    
        Transaction::get_ids_by_hash(&read_tx, &first_transaction.hash)?;
        Transaction::get_by_hash(&read_tx, &first_transaction.hash)?;
        Transaction::take(&read_tx, 100)?;
        Transaction::get(&read_tx, &first_transaction.id)?;
        Transaction::range(&read_tx, &first_transaction.id, &last_transaction.id, None)?;
        Transaction::get_utxos(&read_tx, &first_transaction.id)?;
        Transaction::get_maybe_value(&read_tx, &first_transaction.id)?;
        Transaction::parent_key(&read_tx, &first_transaction.id)?;
        Transaction::stream_ids_by_hash(&read_tx, &first_transaction.hash)?.try_collect::<Vec<BlockPointer>>().await?;
        Transaction::stream_by_hash(storage.begin_read()?, first_transaction.hash.clone(), None)?.try_collect::<Vec<Transaction>>().await?;
        Transaction::stream_range(storage.begin_read()?, first_transaction.id, last_transaction.id, None)?.try_collect::<Vec<Transaction>>().await?;
    
        let transaction_infos = Transaction::table_info(Arc::clone(&storage))?;
        println!("
Transaction persisted with tables :");
        for info in transaction_infos {
            println!("{}", serde_json::to_string_pretty(&info).unwrap());
        }
    
        let first_utxo = Utxo::first(&read_tx)?.unwrap();
        let last_utxo = Utxo::last(&read_tx)?.unwrap();
    
        Utxo::get_by_address(&read_tx, &first_utxo.address)?;
        Utxo::get_ids_by_address(&read_tx, &first_utxo.address)?;
        Utxo::take(&read_tx, 100)?;
        Utxo::get(&read_tx, &first_utxo.id)?;
        Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id, None)?;
        Utxo::get_assets(&read_tx, &first_utxo.id)?;
        Utxo::parent_key(&read_tx, &first_utxo.id)?;
        Utxo::stream_ids_by_address(&read_tx, &first_utxo.address)?.try_collect::<Vec<TransactionPointer>>().await?;
        Utxo::stream_range(storage.begin_read()?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_address(storage.begin_read()?, first_utxo.address.clone(), None)?.try_collect::<Vec<Utxo>>().await?;
        // even streaming parents is possible
        Utxo::stream_transactions_by_address(storage.begin_read()?, first_utxo.address, None)?.try_collect::<Vec<Transaction>>().await?;
    
        let utxo_infos = Utxo::table_info(Arc::clone(&storage))?;
        println!("
Utxo persisted with tables :");
        for info in utxo_infos {
            println!("{}", serde_json::to_string_pretty(&info).unwrap());
        }
    
        let first_asset = Asset::first(&read_tx)?.unwrap();
        let last_asset = Asset::last(&read_tx)?.unwrap();
    
        Asset::get_by_name(&read_tx, &first_asset.name)?;
        Asset::take(&read_tx, 100)?;
        Asset::get(&read_tx, &first_asset.id)?;
        Asset::range(&read_tx, &first_asset.id, &last_asset.id, None)?;
        Asset::parent_key(&read_tx, &first_asset.id)?;
        Asset::stream_by_name(storage.begin_read()?, first_asset.name.clone(), None)?.try_collect::<Vec<Asset>>().await?;
        Asset::stream_range(storage.begin_read()?, first_asset.id, last_asset.id, None)?.try_collect::<Vec<Asset>>().await?;
        // even streaming parents is possible
        Asset::stream_utxos_by_name(storage.begin_read()?, first_asset.name, None)?.try_collect::<Vec<Utxo>>().await?;
    
        let asset_infos = Asset::table_info(Arc::clone(&storage))?;
        println!("
Asset persisted with tables :");
        for info in asset_infos {
            println!("{}", serde_json::to_string_pretty(&info).unwrap());
        }
    
    
        println!("
Deleting blocks:");
        for block in blocks.iter() {
            Block::delete_and_commit(Arc::clone(&storage), &block.height)?;
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
model_v1::block::_store                                             894
model_v1::block::_store_and_commit                                  901
model_v1::block::_store_many                                        923
model_v1::transaction::_store                                      1283
model_v1::transaction::_store_and_commit                           1304
model_v1::transaction::_store_many                                 1494
model_v1::header::_store_and_commit                                2221
model_v1::header::_store                                           2256
model_v1::header::_store_many                                      2256
model_v1::utxo::_store_and_commit                                  2309
model_v1::utxo::_store                                             2312
model_v1::utxo::_store_many                                        2414
model_v1::block::_tail                                             3568
model_v1::block::_take                                             3571
model_v1::asset::_store_and_commit                                 3859
model_v1::asset::_store_many                                       3876
model_v1::asset::_store                                            3889
model_v1::maybevalue::_store_many                                  4618
model_v1::maybevalue::_store_and_commit                            4729
model_v1::maybevalue::_store                                       4904
model_v1::inputref::_store_many                                    5463
model_v1::block::_delete_and_commit                                5502
model_v1::inputref::_store                                         5818
model_v1::inputref::_store_and_commit                              5959
model_v1::transaction::_delete_and_commit                          6272
model_v1::block::_first                                            7130
model_v1::block::_get                                              7133
model_v1::block::_last                                             7147
model_v1::utxo::_delete_and_commit                                 7156
model_v1::block::_get_transactions                                 7401
model_v1::asset::_delete_and_commit                                7556
model_v1::maybevalue::_delete_and_commit                           7818
model_v1::header::_delete_and_commit                               7874
model_v1::inputref::_delete_and_commit                             8957
model_v1::transaction::_take                                      11600
model_v1::transaction::_tail                                      11669
model_v1::transaction::_get_by_hash                               23210
model_v1::transaction::_last                                      23399
model_v1::transaction::_get                                       23475
model_v1::transaction::_first                                     23507
model_v1::transaction::_get_utxos                                 26362
model_v1::block::_range                                           34038
model_v1::block::_stream_range                                    34517
model_v1::transaction::_stream_blocks_by_hash                     38383
model_v1::block::_filter                                          39206
model_v1::utxo::_tail                                             43774
model_v1::utxo::_take                                             44061
model_v1::transaction::_range                                     51246
model_v1::transaction::_stream_range                              52765
model_v1::transaction::_stream_by_hash                            55424
model_v1::utxo::_stream_transactions_by_address                   59035
model_v1::transaction::_filter                                    59882
model_v1::utxo::_get_by_address                                   80777
model_v1::utxo::_get                                              88345
model_v1::utxo::_first                                            88845
model_v1::utxo::_last                                             90490
model_v1::utxo::_range                                            93991
model_v1::utxo::_stream_by_address                                96822
model_v1::utxo::_stream_range                                     97394
model_v1::asset::_stream_utxos_by_name                           103102
model_v1::utxo::_filter                                          117393
model_v1::utxo::_get_assets                                      122062
model_v1::header::_take                                          135208
model_v1::header::_tail                                          135398
model_v1::header::_stream_range_by_duration                      145884
model_v1::header::_stream_range_by_timestamp                     148527
model_v1::asset::_tail                                           167293
model_v1::header::_range                                         176702
model_v1::asset::_take                                           185088
model_v1::header::_stream_range                                  189500
model_v1::asset::_range                                          209492
model_v1::header::_stream_by_hash                                213167
model_v1::header::_stream_by_prev_hash                           216575
model_v1::asset::_stream_by_name                                 218244
model_v1::header::_stream_by_duration                            220706
model_v1::header::_stream_by_timestamp                           222327
model_v1::asset::_stream_range                                   223365
model_v1::header::_range_by_duration                             235722
model_v1::header::_range_by_timestamp                            238706
model_v1::block::_get_header                                     243233
model_v1::header::_get_by_hash                                   245378
model_v1::header::_get_by_prev_hash                              247946
model_v1::header::_get_by_duration                               249147
model_v1::header::_get_by_timestamp                              251312
model_v1::header::_filter                                        269700
model_v1::header::_last                                          271399
model_v1::asset::_get_by_name                                    272755
model_v1::header::_get                                           272833
model_v1::header::_first                                         275249
model_v1::maybevalue::_range                                     317324
model_v1::maybevalue::_stream_range                              352940
model_v1::asset::_filter                                         360507
model_v1::asset::_get                                            364494
model_v1::asset::_first                                          371912
model_v1::asset::_last                                           372366
model_v1::maybevalue::_tail                                      393702
model_v1::maybevalue::_take                                      430008
model_v1::utxo::_stream_ids_by_address                           460732
model_v1::asset::_stream_ids_by_name                             487346
model_v1::maybevalue::_stream_by_hash                            512164
model_v1::utxo::_get_ids_by_address                              565867
model_v1::inputref::_stream_range                                590333
model_v1::asset::_get_ids_by_name                                615400
model_v1::maybevalue::_get_by_hash                               627333
model_v1::inputref::_pk_range                                    679569
model_v1::transaction::_get_inputs                               690174
model_v1::utxo::_pk_range                                        693775
model_v1::transaction::_pk_range                                 702923
model_v1::asset::_pk_range                                       707499
model_v1::transaction::_get_maybe_value                          720228
model_v1::inputref::_tail                                        738307
model_v1::block::_pk_range                                       744751
model_v1::maybevalue::_pk_range                                  767713
model_v1::transaction::_stream_ids_by_hash                       783282
model_v1::header::_pk_range                                      792952
model_v1::maybevalue::_filter                                    811445
model_v1::inputref::_range                                       813762
model_v1::header::_stream_heights_by_hash                        814153
model_v1::maybevalue::_get                                       814160
model_v1::maybevalue::_last                                      842325
model_v1::maybevalue::_first                                     845030
model_v1::maybevalue::_stream_ids_by_hash                        855249
model_v1::header::_stream_heights_by_prev_hash                   856274
model_v1::header::_stream_heights_by_duration                    890543
model_v1::header::_stream_heights_by_timestamp                   892148
model_v1::inputref::_take                                        989266
model_v1::transaction::_get_ids_by_hash                         1088009
model_v1::header::_get_heights_by_hash                          1088566
model_v1::maybevalue::_get_ids_by_hash                          1127091
model_v1::header::_get_heights_by_prev_hash                     1162493
model_v1::header::_get_heights_by_timestamp                     1240449
model_v1::header::_get_heights_by_duration                      1256518
model_v1::transaction::_exists                                  1551566
model_v1::utxo::_exists                                         1596704
model_v1::inputref::_exists                                     1644385
model_v1::block::_exists                                        1657248
model_v1::inputref::_get                                        1660909
model_v1::inputref::_filter                                     1689931
model_v1::asset::_exists                                        1713003
model_v1::inputref::_first                                      1848292
model_v1::inputref::_last                                       1852332
model_v1::maybevalue::_exists                                   1865080
model_v1::header::_exists                                       1922744
```
<!-- END_BENCH -->


## Chain

[chain](./chain) syncs blockchains with nodes :
- [demo](./examples/demo)
- [btc](./examples/btc)
- [cardano](./examples/cardano)
- [ergo](./examples/ergo)

### ⏱️ Syncing performance Summary

Hand-made criterion benchmarks [deployed](https://pragmaxim-com.github.io/redbit/report/index.html).

Indexing speed in logs is the **average**, for example, the first ~ 100k **bitcoin** blocks with just one Tx have 
lower in/out indexing throughput because the block is indexed into ~ 24 tables in total.

If node and indexer each uses its own SSD, then the throughput reaches :

 - 2.0GHz & NVMe on PCIe Gen3 : `~ 9 000 Inputs+outputs / s`
 - 3.0GHz & NVMe on PCIe Gen4 : `~ 15 000 Inputs+outputs / s`
 - 4.0GHz & NVMe on PCIe Gen5 : `~ 28 000 Inputs+outputs / s`

In a nutshell, whole bitcoin up to height ~ 0.9M can be indexed in less than 4 days on a PCIe Gen5 SSD with 4.0GHz CPU.
