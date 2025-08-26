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
        let write_tx = storage.begin_write()?;
        Block::store_many(&write_tx, blocks)?;
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
            println!("{}", serde_json::to_string_pretty(&info)?);
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
            println!("{}", serde_json::to_string_pretty(&info)?);
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
        Transaction::stream_ids_by_hash(storage.begin_read()?, first_transaction.hash.clone())?.try_collect::<Vec<BlockPointer>>().await?;
        Transaction::stream_by_hash(storage.begin_read()?, first_transaction.hash.clone(), None)?.try_collect::<Vec<Transaction>>().await?;
        Transaction::stream_range(storage.begin_read()?, first_transaction.id, last_transaction.id, None)?.try_collect::<Vec<Transaction>>().await?;
    
        let transaction_infos = Transaction::table_info(Arc::clone(&storage))?;
        println!("
Transaction persisted with tables :");
        for info in transaction_infos {
            println!("{}", serde_json::to_string_pretty(&info)?);
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
        Utxo::stream_ids_by_address(storage.begin_read()?, first_utxo.address.clone())?.try_collect::<Vec<TransactionPointer>>().await?;
        Utxo::stream_range(storage.begin_read()?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_address(storage.begin_read()?, first_utxo.address.clone(), None)?.try_collect::<Vec<Utxo>>().await?;
        // even streaming parents is possible
        Utxo::stream_transactions_by_address(storage.begin_read()?, first_utxo.address, None)?.try_collect::<Vec<Transaction>>().await?;
    
        let utxo_infos = Utxo::table_info(Arc::clone(&storage))?;
        println!("
Utxo persisted with tables :");
        for info in utxo_infos {
            println!("{}", serde_json::to_string_pretty(&info)?);
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
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
    
    
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
model_v1::block::_store_and_commit                                  958
model_v1::block::_store                                             960
model_v1::block::_store_many                                        998
model_v1::transaction::_store                                      1217
model_v1::transaction::_store_and_commit                           1255
model_v1::transaction::_store_many                                 1333
model_v1::header::_store_and_commit                                2143
model_v1::header::_store                                           2204
model_v1::utxo::_store                                             2209
model_v1::utxo::_store_and_commit                                  2217
model_v1::header::_store_many                                      2230
model_v1::utxo::_store_many                                        2318
model_v1::asset::_store                                            3798
model_v1::asset::_store_and_commit                                 3831
model_v1::asset::_store_many                                       3834
model_v1::maybevalue::_store                                       4721
model_v1::maybevalue::_store_and_commit                            4740
model_v1::maybevalue::_store_many                                  4878
model_v1::block::_tail                                             5340
model_v1::block::_take                                             5413
model_v1::inputref::_store_many                                    5733
model_v1::inputref::_store                                         5799
model_v1::inputref::_store_and_commit                              5865
model_v1::block::_delete_and_commit                                6554
model_v1::transaction::_delete_and_commit                          7174
model_v1::asset::_delete_and_commit                                7865
model_v1::utxo::_delete_and_commit                                 8327
model_v1::inputref::_delete_and_commit                             9227
model_v1::maybevalue::_delete_and_commit                           9361
model_v1::header::_delete_and_commit                               9641
model_v1::block::_get                                             10744
model_v1::block::_last                                            10811
model_v1::block::_first                                           10847
model_v1::block::_get_transactions                                11264
model_v1::transaction::_tail                                      16946
model_v1::transaction::_take                                      17000
model_v1::transaction::_get_by_hash                               33895
model_v1::transaction::_first                                     34121
model_v1::transaction::_last                                      34247
model_v1::transaction::_get                                       34485
model_v1::transaction::_get_utxos                                 39612
model_v1::block::_range                                           51340
model_v1::block::_stream_range                                    51872
model_v1::transaction::_stream_blocks_by_hash                     55492
model_v1::block::_filter                                          59128
model_v1::utxo::_tail                                             60931
model_v1::utxo::_take                                             62902
model_v1::transaction::_range                                     74916
model_v1::transaction::_stream_range                              75112
model_v1::transaction::_stream_by_hash                            79989
model_v1::utxo::_stream_transactions_by_address                   80860
model_v1::transaction::_filter                                    86524
model_v1::utxo::_get_by_address                                  117738
model_v1::utxo::_get                                             128211
model_v1::utxo::_first                                           128492
model_v1::utxo::_last                                            129019
model_v1::utxo::_stream_range                                    131140
model_v1::utxo::_range                                           132034
model_v1::utxo::_stream_by_address                               135774
model_v1::asset::_stream_utxos_by_name                           144289
model_v1::utxo::_filter                                          167554
model_v1::utxo::_get_assets                                      173390
model_v1::header::_stream_range_by_duration                      193327
model_v1::header::_stream_range_by_timestamp                     198681
model_v1::header::_take                                          201010
model_v1::header::_tail                                          203296
model_v1::asset::_tail                                           234022
model_v1::asset::_take                                           258663
model_v1::header::_stream_range                                  260641
model_v1::header::_range                                         262347
model_v1::asset::_range                                          271278
model_v1::asset::_stream_range                                   283969
model_v1::header::_stream_by_hash                                305680
model_v1::header::_stream_by_duration                            306146
model_v1::header::_stream_by_timestamp                           309046
model_v1::header::_stream_by_prev_hash                           310022
model_v1::asset::_stream_by_name                                 320497
model_v1::header::_range_by_duration                             324067
model_v1::header::_range_by_timestamp                            346232
model_v1::header::_get_by_hash                                   372133
model_v1::header::_get_by_duration                               372146
model_v1::header::_get_by_prev_hash                              377735
model_v1::header::_get_by_timestamp                              378827
model_v1::header::_filter                                        385223
model_v1::block::_get_header                                     386674
model_v1::asset::_get_by_name                                    392639
model_v1::header::_first                                         401781
model_v1::header::_last                                          406337
model_v1::header::_get                                           410889
model_v1::maybevalue::_range                                     435897
model_v1::maybevalue::_stream_range                              468553
model_v1::asset::_filter                                         493252
model_v1::asset::_get                                            510655
model_v1::asset::_last                                           519041
model_v1::asset::_first                                          521763
model_v1::maybevalue::_tail                                      527894
model_v1::maybevalue::_take                                      622231
model_v1::inputref::_stream_range                                693164
model_v1::maybevalue::_stream_by_hash                            715057
model_v1::utxo::_stream_ids_by_address                           722256
model_v1::asset::_stream_ids_by_name                             746419
model_v1::asset::_pk_range                                       783404
model_v1::utxo::_pk_range                                        802652
model_v1::inputref::_pk_range                                    814803
model_v1::maybevalue::_pk_range                                  848320
model_v1::transaction::_pk_range                                 851296
model_v1::block::_pk_range                                       923054
model_v1::header::_pk_range                                      939161
model_v1::inputref::_tail                                        967174
model_v1::transaction::_get_inputs                               991798
model_v1::utxo::_get_ids_by_address                             1017853
model_v1::asset::_get_ids_by_name                               1028965
model_v1::transaction::_get_maybe_value                         1036699
model_v1::header::_stream_heights_by_duration                   1052576
model_v1::inputref::_range                                      1057194
model_v1::maybevalue::_get_by_hash                              1065712
model_v1::header::_stream_heights_by_hash                       1071662
model_v1::header::_stream_heights_by_prev_hash                  1123608
model_v1::transaction::_stream_ids_by_hash                      1126672
model_v1::header::_stream_heights_by_timestamp                  1131657
model_v1::maybevalue::_filter                                   1182578
model_v1::maybevalue::_get                                      1191881
model_v1::maybevalue::_first                                    1239987
model_v1::maybevalue::_stream_ids_by_hash                       1254469
model_v1::inputref::_take                                       1256629
model_v1::maybevalue::_last                                     1261400
model_v1::header::_get_heights_by_duration                      1616057
model_v1::header::_get_heights_by_hash                          1640420
model_v1::transaction::_get_ids_by_hash                         1681181
model_v1::header::_get_heights_by_prev_hash                     1742191
model_v1::header::_get_heights_by_timestamp                     1824784
model_v1::maybevalue::_get_ids_by_hash                          1979336
model_v1::inputref::_exists                                     2220249
model_v1::inputref::_get                                        2234437
model_v1::inputref::_filter                                     2263468
model_v1::block::_exists                                        2266135
model_v1::utxo::_exists                                         2303511
model_v1::asset::_exists                                        2326988
model_v1::transaction::_exists                                  2441525
model_v1::maybevalue::_exists                                   2461175
model_v1::inputref::_last                                       2467065
model_v1::header::_exists                                       2485337
model_v1::inputref::_first                                      2495259
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

 - 2.0GHz & NVMe on PCIe Gen3 : `~ 11 000 Inputs+outputs / s`
 - 3.0GHz & NVMe on PCIe Gen4 : `~ 18 000 Inputs+outputs / s`
 - 4.0GHz & NVMe on PCIe Gen5 : `~ 26 000 Inputs+outputs / s`

In a nutshell, whole bitcoin up to height ~ 0.9M can be indexed in 3-4 days on a PCIe Gen5 SSD with 4.0GHz CPU.
