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

```
cd examples/utxo
cargo test                          # to let all the self-generated tests run
cargo test --features integration   # to let http layer self-generated tests run
cargo bench                         # to run benchmarks
cargo run                           # to run the demo example and start the server
```

Check the [redbit-ui](http://github.com/pragmaxim-com/redbit-ui) for frontend dev.

The utxo example has close to 500 frontend/backend derived tests and 130 benchmarks, so that if any redbit app derived from the definition compiles,
it is transparent, well tested and benched already.

### Chain  

[chain](./chain) syncs blockchains with nodes : 
 - [btc](./examples/btc)
 - [cardano](./examples/cardano)
 - [ergo](./examples/ergo)

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
    
        fn resolve_tx_inputs(&self, read_tx: &StorageReadTx, block: &mut Block) -> Result<(), ChainSyncError> {
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

Performance wise, check 🔥[flamegraph](https://rawcdn.githack.com/pragmaxim-com/redbit/refs/heads/master/flamegraph.svg).
The demo example persists data into 30 tables to allow for rich querying.

### ⏱️ Performance Summary

Indexing speed in logs is the **average**, for example, the first ~ 100k **bitcoin** blocks with just one Tx have 
lower in/out indexing throughput because the block is indexed into ~ 24 tables in total.

If node and indexer each uses its own SSD, then the throughput reaches :

 - 2.0GHz & NVMe on PCIe Gen3 : `~ 9 000 Inputs+outputs / s`
 - 3.0GHz & NVMe on PCIe Gen4 : `~ 15 000 Inputs+outputs / s`
 - 4.0GHz & NVMe on PCIe Gen5 : `~ 28 000 Inputs+outputs / s`

In a nutshell, whole bitcoin up to height ~ 0.9M can be indexed in less than 4 days on a PCIe Gen5 SSD with 4.0GHz CPU.

### Flamegraphs

```
cargo flamegraph --bin target/release/ergo --release
cargo flamegraph --bin target/release/cargo --release
cargo flamegraph --bin target/release/btc --release
```

### ⏱ Benchmarks (results from github servers)

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
model_v1::block::_store_and_commit                                  895
model_v1::block::_store                                             904
model_v1::block::_store_many                                        919
model_v1::transaction::_store                                      1267
model_v1::transaction::_store_and_commit                           1359
model_v1::transaction::_store_many                                 1620
model_v1::header::_store                                           2284
model_v1::header::_store_and_commit                                2295
model_v1::header::_store_many                                      2309
model_v1::utxo::_store                                             2318
model_v1::utxo::_store_many                                        2330
model_v1::utxo::_store_and_commit                                  2343
model_v1::block::_tail                                             3502
model_v1::block::_take                                             3505
model_v1::asset::_store_and_commit                                 3828
model_v1::asset::_store_many                                       3859
model_v1::asset::_store                                            3901
model_v1::maybevalue::_store_and_commit                            4249
model_v1::maybevalue::_store                                       4783
model_v1::maybevalue::_store_many                                  4831
model_v1::inputref::_store                                         5334
model_v1::block::_delete_and_commit                                5346
model_v1::inputref::_store_many                                    5383
model_v1::inputref::_store_and_commit                              5441
model_v1::transaction::_delete_and_commit                          6613
model_v1::block::_first                                            6998
model_v1::block::_get                                              7013
model_v1::block::_last                                             7053
model_v1::header::_delete_and_commit                               7335
model_v1::block::_get_transactions                                 7351
model_v1::utxo::_delete_and_commit                                 7479
model_v1::asset::_delete_and_commit                                7573
model_v1::maybevalue::_delete_and_commit                           8004
model_v1::inputref::_delete_and_commit                             9075
model_v1::transaction::_tail                                      11516
model_v1::transaction::_take                                      11517
model_v1::transaction::_get_by_hash                               22934
model_v1::transaction::_last                                      22956
model_v1::transaction::_first                                     22963
model_v1::transaction::_get                                       23090
model_v1::transaction::_get_utxos                                 25972
model_v1::block::_range                                           33995
model_v1::block::_stream_range                                    34500
model_v1::transaction::_stream_blocks_by_hash                     38087
model_v1::block::_filter                                          38750
model_v1::utxo::_tail                                             42278
model_v1::utxo::_take                                             42649
model_v1::transaction::_range                                     50926
model_v1::transaction::_stream_range                              52813
model_v1::transaction::_stream_by_hash                            55065
model_v1::utxo::_stream_transactions_by_address                   56836
model_v1::transaction::_filter                                    58630
model_v1::utxo::_get_by_address                                   76177
model_v1::utxo::_first                                            82783
model_v1::utxo::_last                                             83773
model_v1::utxo::_get                                              84688
model_v1::utxo::_range                                            90079
model_v1::utxo::_stream_range                                     90317
model_v1::utxo::_stream_by_address                                91214
model_v1::asset::_stream_utxos_by_name                            99597
model_v1::utxo::_filter                                          109794
model_v1::utxo::_get_assets                                      113759
model_v1::header::_tail                                          132188
model_v1::header::_take                                          133638
model_v1::header::_stream_range_by_duration                      135933
model_v1::header::_stream_range_by_timestamp                     135995
model_v1::asset::_tail                                           168571
model_v1::header::_range                                         174456
model_v1::asset::_take                                           176117
model_v1::header::_stream_range                                  181352
model_v1::asset::_range                                          198674
model_v1::header::_stream_by_hash                                208557
model_v1::header::_stream_by_prev_hash                           209848
model_v1::asset::_stream_range                                   210426
model_v1::header::_stream_by_duration                            211494
model_v1::asset::_stream_by_name                                 213683
model_v1::header::_stream_by_timestamp                           213953
model_v1::header::_get_by_hash                                   226101
model_v1::header::_get_by_prev_hash                              227181
model_v1::header::_range_by_duration                             228766
model_v1::header::_range_by_timestamp                            231351
model_v1::header::_get_by_duration                               232615
model_v1::header::_get_by_timestamp                              233111
model_v1::block::_get_header                                     242025
model_v1::header::_filter                                        248262
model_v1::asset::_get_by_name                                    252723
model_v1::header::_get                                           257700
model_v1::header::_last                                          263217
model_v1::header::_first                                         264052
model_v1::maybevalue::_range                                     306162
model_v1::asset::_filter                                         339444
model_v1::asset::_get                                            339690
model_v1::maybevalue::_stream_range                              351833
model_v1::asset::_last                                           354772
model_v1::asset::_first                                          355518
model_v1::maybevalue::_tail                                      389211
model_v1::maybevalue::_take                                      423191
model_v1::utxo::_stream_ids_by_address                           431520
model_v1::maybevalue::_stream_by_hash                            464967
model_v1::asset::_stream_ids_by_name                             467347
model_v1::utxo::_get_ids_by_address                              531341
model_v1::inputref::_stream_range                                562325
model_v1::maybevalue::_get_by_hash                               602475
model_v1::asset::_get_ids_by_name                                613745
model_v1::inputref::_pk_range                                    618353
model_v1::utxo::_pk_range                                        626080
model_v1::transaction::_pk_range                                 644326
model_v1::asset::_pk_range                                       656758
model_v1::block::_pk_range                                       681863
model_v1::maybevalue::_pk_range                                  690861
model_v1::transaction::_get_inputs                               707849
model_v1::transaction::_stream_ids_by_hash                       720929
model_v1::transaction::_get_maybe_value                          721897
model_v1::inputref::_tail                                        738662
model_v1::header::_pk_range                                      739568
model_v1::maybevalue::_get                                       778125
model_v1::header::_stream_heights_by_hash                        781794
model_v1::maybevalue::_filter                                    788793
model_v1::maybevalue::_stream_ids_by_hash                        789840
model_v1::header::_stream_heights_by_prev_hash                   809003
model_v1::header::_stream_heights_by_duration                    813015
model_v1::inputref::_range                                       822335
model_v1::maybevalue::_first                                     832931
model_v1::header::_stream_heights_by_timestamp                   836050
model_v1::maybevalue::_last                                      838610
model_v1::inputref::_take                                        978148
model_v1::transaction::_get_ids_by_hash                         1049847
model_v1::header::_get_heights_by_hash                          1059053
model_v1::maybevalue::_get_ids_by_hash                          1082216
model_v1::header::_get_heights_by_prev_hash                     1105608
model_v1::header::_get_heights_by_duration                      1181461
model_v1::header::_get_heights_by_timestamp                     1222449
model_v1::transaction::_exists                                  1380929
model_v1::block::_exists                                        1555960
model_v1::utxo::_exists                                         1576119
model_v1::inputref::_get                                        1597061
model_v1::inputref::_filter                                     1600743
model_v1::inputref::_exists                                     1601204
model_v1::maybevalue::_exists                                   1616893
model_v1::asset::_exists                                        1668975
model_v1::inputref::_last                                       1750884
model_v1::inputref::_first                                      1760161
model_v1::header::_exists                                       1827519
```
<!-- END_BENCH -->
