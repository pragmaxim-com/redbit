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
model_v1::block::_store_many                                        688
model_v1::block::_store_and_commit                                  766
model_v1::block::_store                                             788
model_v1::transaction::_store_many                                 1251
model_v1::transaction::_store                                      1356
model_v1::transaction::_store_and_commit                           1388
model_v1::header::_store_and_commit                                1972
model_v1::utxo::_store_many                                        2088
model_v1::asset::_store                                            2158
model_v1::header::_store                                           2160
model_v1::header::_store_many                                      2214
model_v1::utxo::_store_and_commit                                  2243
model_v1::utxo::_store                                             2317
model_v1::header::_delete_and_commit                               2630
model_v1::asset::_store_many                                       2747
model_v1::asset::_store_and_commit                                 2914
model_v1::block::_take                                             3164
model_v1::maybevalue::_store_and_commit                            3186
model_v1::block::_tail                                             3238
model_v1::maybevalue::_store_many                                  3258
model_v1::maybevalue::_store                                       3436
model_v1::inputref::_store                                         3465
model_v1::inputref::_store_many                                    3873
model_v1::inputref::_store_and_commit                              3948
model_v1::block::_delete_and_commit                                4671
model_v1::utxo::_delete_and_commit                                 5313
model_v1::transaction::_delete_and_commit                          5383
model_v1::inputref::_delete_and_commit                             5801
model_v1::block::_last                                             6354
model_v1::asset::_delete_and_commit                                6406
model_v1::block::_first                                            6522
model_v1::block::_get                                              6605
model_v1::block::_get_transactions                                 6684
model_v1::maybevalue::_delete_and_commit                           6857
model_v1::transaction::_take                                      10586
model_v1::transaction::_tail                                      10646
model_v1::transaction::_get_by_hash                               20784
model_v1::transaction::_last                                      20819
model_v1::transaction::_first                                     20841
model_v1::transaction::_get                                       21021
model_v1::transaction::_get_utxos                                 23657
model_v1::block::_range                                           30864
model_v1::block::_stream_range                                    31843
model_v1::transaction::_stream_blocks_by_hash                     34988
model_v1::block::_filter                                          35809
model_v1::utxo::_tail                                             39334
model_v1::utxo::_take                                             39798
model_v1::transaction::_range                                     47473
model_v1::transaction::_stream_range                              48309
model_v1::transaction::_stream_by_hash                            50856
model_v1::utxo::_stream_transactions_by_address                   52388
model_v1::transaction::_filter                                    54553
model_v1::utxo::_get_by_address                                   72621
model_v1::utxo::_get                                              80121
model_v1::utxo::_first                                            80902
model_v1::utxo::_last                                             81346
model_v1::utxo::_stream_by_address                                86024
model_v1::utxo::_range                                            87413
model_v1::utxo::_stream_range                                     87671
model_v1::asset::_stream_utxos_by_name                            93463
model_v1::utxo::_filter                                          105902
model_v1::utxo::_get_assets                                      110820
model_v1::header::_tail                                          120152
model_v1::header::_take                                          123051
model_v1::header::_stream_range_by_duration                      128501
model_v1::header::_stream_range_by_timestamp                     129761
model_v1::header::_range                                         156442
model_v1::asset::_tail                                           160419
model_v1::header::_stream_range                                  162174
model_v1::asset::_take                                           164516
model_v1::header::_stream_by_hash                                180957
model_v1::header::_stream_by_prev_hash                           183492
model_v1::header::_stream_by_duration                            188781
model_v1::header::_stream_by_timestamp                           192689
model_v1::asset::_range                                          194598
model_v1::asset::_stream_range                                   199427
model_v1::asset::_stream_by_name                                 201456
model_v1::header::_range_by_duration                             207384
model_v1::header::_range_by_timestamp                            211154
model_v1::block::_get_header                                     216047
model_v1::header::_get_by_hash                                   217208
model_v1::header::_get_by_prev_hash                              218525
model_v1::header::_get_by_duration                               218969
model_v1::header::_get_by_timestamp                              226366
model_v1::header::_filter                                        235224
model_v1::header::_get                                           241579
model_v1::asset::_get_by_name                                    241654
model_v1::header::_first                                         245177
model_v1::header::_last                                          246300
model_v1::maybevalue::_range                                     299411
model_v1::maybevalue::_stream_range                              323618
model_v1::asset::_filter                                         328925
model_v1::asset::_get                                            337595
model_v1::asset::_last                                           344024
model_v1::asset::_first                                          344332
model_v1::maybevalue::_tail                                      365376
model_v1::maybevalue::_take                                      382126
model_v1::utxo::_stream_ids_by_address                           412006
model_v1::asset::_stream_ids_by_name                             442118
model_v1::maybevalue::_stream_by_hash                            445204
model_v1::utxo::_get_ids_by_address                              507105
model_v1::inputref::_stream_range                                514724
model_v1::asset::_get_ids_by_name                                567888
model_v1::maybevalue::_get_by_hash                               582554
model_v1::inputref::_pk_range                                    600781
model_v1::transaction::_pk_range                                 626288
model_v1::utxo::_pk_range                                        643008
model_v1::asset::_pk_range                                       643981
model_v1::transaction::_get_inputs                               661188
model_v1::transaction::_get_maybe_value                          665624
model_v1::block::_pk_range                                       678970
model_v1::inputref::_tail                                        687734
model_v1::transaction::_stream_ids_by_hash                       711642
model_v1::maybevalue::_pk_range                                  714735
model_v1::header::_pk_range                                      716055
model_v1::header::_stream_heights_by_hash                        725584
model_v1::inputref::_range                                       730508
model_v1::maybevalue::_get                                       737050
model_v1::maybevalue::_filter                                    743014
model_v1::maybevalue::_stream_ids_by_hash                        749316
model_v1::maybevalue::_first                                     762695
model_v1::header::_stream_heights_by_prev_hash                   773120
model_v1::maybevalue::_last                                      776832
model_v1::header::_stream_heights_by_duration                    801847
model_v1::header::_stream_heights_by_timestamp                   815202
model_v1::inputref::_take                                        909438
model_v1::transaction::_get_ids_by_hash                          939241
model_v1::header::_get_heights_by_hash                           989492
model_v1::header::_get_heights_by_prev_hash                     1034383
model_v1::maybevalue::_get_ids_by_hash                          1046638
model_v1::header::_get_heights_by_duration                      1099082
model_v1::header::_get_heights_by_timestamp                     1156805
model_v1::transaction::_exists                                  1288394
model_v1::inputref::_get                                        1423873
model_v1::inputref::_exists                                     1465201
model_v1::inputref::_filter                                     1470869
model_v1::block::_exists                                        1485730
model_v1::utxo::_exists                                         1490046
model_v1::inputref::_first                                      1597776
model_v1::inputref::_last                                       1599079
model_v1::asset::_exists                                        1633613
model_v1::maybevalue::_exists                                   1700507
model_v1::header::_exists                                       1730852
```
<!-- END_BENCH -->
