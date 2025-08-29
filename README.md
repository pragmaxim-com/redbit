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
        #[column(dictionary)]
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
        #[column(dictionary)]
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
    
        fn resolve_tx_inputs(&self, tx_context: &BlockReadTxContext, block: &mut Block) -> Result<(), ChainError> {
            for tx in &mut block.transactions {
                for transient_input in tx.transient_inputs.iter_mut() {
                    let tx_pointers = Transaction::get_ids_by_hash(&tx_context.transactions, &transient_input.tx_hash)?;
    
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
model_v1::block::_store_and_commit                                  959
model_v1::block::_store                                             966
model_v1::block::_store_many                                       1004
model_v1::transaction::_store                                      1331
model_v1::transaction::_store_and_commit                           1370
model_v1::transaction::_store_many                                 1530
model_v1::header::_store_and_commit                                2311
model_v1::header::_store_many                                      2362
model_v1::header::_store                                           2370
model_v1::utxo::_store_and_commit                                  2377
model_v1::utxo::_store                                             2445
model_v1::utxo::_store_many                                        2501
model_v1::asset::_store                                            3848
model_v1::asset::_store_many                                       3879
model_v1::asset::_store_and_commit                                 3884
model_v1::maybevalue::_store_and_commit                            4508
model_v1::maybevalue::_store_many                                  4510
model_v1::maybevalue::_store                                       4582
model_v1::inputref::_store                                         4871
model_v1::inputref::_store_and_commit                              5053
model_v1::utxo::_pk_range                                          5214
model_v1::block::_delete_and_commit                                5301
model_v1::block::_pk_range                                         5374
model_v1::transaction::_pk_range                                   5470
model_v1::transaction::_delete_and_commit                          5558
model_v1::inputref::_store_many                                    5791
model_v1::header::_pk_range                                        5831
model_v1::header::_delete_and_commit                               6333
model_v1::utxo::_delete_and_commit                                 6680
model_v1::maybevalue::_pk_range                                    6765
model_v1::maybevalue::_delete_and_commit                           7335
model_v1::inputref::_delete_and_commit                             7408
model_v1::asset::_pk_range                                         8269
model_v1::asset::_delete_and_commit                                9030
model_v1::inputref::_pk_range                                      9937
model_v1::block::_take                                            18367
model_v1::block::_tail                                            18390
model_v1::block::_get_transactions                                37545
model_v1::block::_get                                             37738
model_v1::block::_first                                           37892
model_v1::block::_last                                            38200
model_v1::block::_stream_range                                    44098
model_v1::transaction::_stream_blocks_by_hash                     45621
model_v1::transaction::_take                                      60679
model_v1::transaction::_tail                                      61288
model_v1::transaction::_stream_range                              66369
model_v1::transaction::_stream_by_hash                            67444
model_v1::utxo::_stream_transactions_by_address                   72325
model_v1::header::_stream_range_by_duration                       91827
model_v1::transaction::_stream_ids_by_hash                       106922
model_v1::utxo::_stream_range                                    117059
model_v1::utxo::_stream_by_address                               119224
model_v1::transaction::_get_by_hash                              123920
model_v1::asset::_stream_utxos_by_name                           124937
model_v1::transaction::_get                                      126325
model_v1::transaction::_first                                    128126
model_v1::transaction::_last                                     129120
model_v1::transaction::_get_utxos                                150142
model_v1::header::_stream_range_by_timestamp                     158248
model_v1::block::_range                                          163308
model_v1::utxo::_stream_ids_by_address                           168509
model_v1::header::_stream_range                                  181646
model_v1::header::_stream_by_duration                            185071
model_v1::header::_stream_by_prev_hash                           188737
model_v1::header::_stream_by_hash                                190022
model_v1::header::_stream_by_timestamp                           191125
model_v1::block::_filter                                         191437
model_v1::transaction::_range                                    214075
model_v1::utxo::_tail                                            216698
model_v1::header::_stream_heights_by_duration                    226074
model_v1::header::_stream_heights_by_hash                        227195
model_v1::header::_stream_heights_by_prev_hash                   228241
model_v1::header::_stream_heights_by_timestamp                   228708
model_v1::utxo::_take                                            233867
model_v1::asset::_stream_range                                   237706
model_v1::asset::_stream_by_name                                 264249
model_v1::transaction::_filter                                   276812
model_v1::asset::_stream_ids_by_name                             328123
model_v1::utxo::_range                                           406641
model_v1::maybevalue::_stream_range                              447551
model_v1::utxo::_get_by_address                                  452016
model_v1::utxo::_first                                           518363
model_v1::utxo::_get                                             522687
model_v1::utxo::_last                                            527409
model_v1::maybevalue::_stream_by_hash                            536193
model_v1::utxo::_filter                                          599204
model_v1::utxo::_get_assets                                      609868
model_v1::maybevalue::_stream_ids_by_hash                        638737
model_v1::inputref::_stream_range                                654789
model_v1::asset::_tail                                           709889
model_v1::asset::_range                                          825730
model_v1::asset::_take                                          1015548
model_v1::header::_tail                                         1062575
model_v1::maybevalue::_tail                                     1077029
model_v1::header::_range                                        1111445
model_v1::header::_take                                         1115275
model_v1::header::_range_by_duration                            1131529
model_v1::maybevalue::_range                                    1211006
model_v1::header::_range_by_timestamp                           1397917
model_v1::inputref::_tail                                       1521746
model_v1::transaction::_get_inputs                              1656864
model_v1::maybevalue::_take                                     1737589
model_v1::inputref::_range                                      1744227
model_v1::asset::_get_by_name                                   1817554
model_v1::header::_get_by_duration                              2108148
model_v1::header::_get_by_hash                                  2271179
model_v1::header::_get_by_timestamp                             2294631
model_v1::header::_get_by_prev_hash                             2349734
model_v1::inputref::_take                                       2440751
model_v1::asset::_filter                                        2558003
model_v1::asset::_get                                           2802298
model_v1::asset::_last                                          2879935
model_v1::header::_filter                                       2922695
model_v1::asset::_first                                         2996524
model_v1::block::_get_header                                    3407387
model_v1::header::_last                                         3550254
model_v1::header::_first                                        3554671
model_v1::header::_get                                          3570536
model_v1::maybevalue::_get_by_hash                              3814901
model_v1::asset::_get_ids_by_name                               3819710
model_v1::utxo::_get_ids_by_address                             3934839
model_v1::header::_get_heights_by_duration                      4468675
model_v1::maybevalue::_get_ids_by_hash                          5502669
model_v1::header::_get_heights_by_hash                          5505092
model_v1::transaction::_get_ids_by_hash                         5538632
model_v1::header::_get_heights_by_prev_hash                     5566689
model_v1::header::_get_heights_by_timestamp                     5575068
model_v1::maybevalue::_filter                                   7750736
model_v1::transaction::_get_maybe_value                         7859781
model_v1::maybevalue::_get                                      7888302
model_v1::maybevalue::_first                                    8144649
model_v1::maybevalue::_last                                     8380122
model_v1::asset::_exists                                       13049719
model_v1::inputref::_filter                                    16350556
model_v1::inputref::_get                                       16401509
model_v1::utxo::_exists                                        16578249
model_v1::inputref::_exists                                    16675004
model_v1::transaction::_exists                                 17161490
model_v1::maybevalue::_exists                                  17730496
model_v1::inputref::_last                                      24073182
model_v1::header::_exists                                      25125628
model_v1::block::_exists                                       25169897
model_v1::inputref::_first                                     25400051
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
