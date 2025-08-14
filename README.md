Built for blazing fast persistence of terra bytes of structured data on a single machine
while offering rich querying capabilities, e.g. bitcoin/blockchain data. Blockchains need rich and often
analytical queries which is done through explorers because indexing speed of even embedded/in-process (not through socket) 
analytical db like [DuckDB](https://duckdb.org/) right on the node would be still slower than a hand-made solution on top of 
[Redb](https://github.com/cberner/redb) or [RocksDb](https://rocksdb.org/).

Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from
[Redb](https://github.com/cberner/redb) using secondary indexes and dictionaries, served by [axum](https://github.com/tokio-rs/axum)
through auto-generated REST API. It maximizes R/W speed while minimizing data size using hierarchical data structures of smart pointers.

### Major Out-of-the-Box Features

✅ Querying and ranging by secondary index \
✅ Optional dictionaries for low cardinality fields + first level cache for building them without overhead \
✅ `One-to-One` / `One-to-Option` / `One-to-Many` entities with cascade read/write/delete \
✅ All goodies including intuitive data ordering without writing custom codecs \
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
✅ Macro derived http rest API at http://127.0.0.1:8000/swagger-ui/ with examples \
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

### Chain syncer  

[chain-syncer](./syncer) syncs example blockchains with nodes : 
 - [btc](./examples/btc)
 - [cardano](./examples/cardano)
 - [ergo](./examples/ergo)

Let's say we want to persist and query blockchain data using Redbit, declare annotated Structs `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    pub use redbit::*;
    use syncer::api::{BlockHeaderLike, BlockLike};
    
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
    #[column]
    #[derive(Copy, Hash)]
    pub struct Timestamp(pub u32);
    
    #[column]
    pub struct TempInputRef {
        pub tx_hash: TxHash,
        pub index: u32,
    }
    
    #[entity]
    pub struct Block {
        #[pk]
        pub height: Height,
        pub header: BlockHeader,
        pub transactions: Vec<Transaction>,
        #[column(transient)]
        pub weight: u32,
    }
    
    #[entity]
    pub struct BlockHeader {
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
        #[column(dictionary(cache = 1000000))]
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
        #[column(dictionary(cache = 1000000))]
        pub name: AssetName,
    }
    
    impl BlockHeaderLike for BlockHeader {
        fn height(&self) -> u32 {
            self.height.0
        }
        fn hash(&self) -> [u8; 32] {
            self.hash.0
        }
        fn prev_hash(&self) -> [u8; 32] {
            self.prev_hash.0
        }
        fn timestamp(&self) -> u32 {
            self.timestamp.0
        }
    }
    
    impl BlockLike for Block {
        type Header = BlockHeader;
        fn header(&self) -> &Self::Header {
            &self.header
        }
        fn weight(&self) -> u32 {
            self.weight
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
    
        let first_block_header = BlockHeader::first(&read_tx)?.unwrap();
        let last_block_header = BlockHeader::last(&read_tx)?.unwrap();
    
        BlockHeader::get_by_hash(&read_tx, &first_block_header.hash)?;
        BlockHeader::get_by_timestamp(&read_tx, &first_block_header.timestamp)?;
        BlockHeader::take(&read_tx, 100)?;
        BlockHeader::get(&read_tx, &first_block_header.height)?;
        BlockHeader::range(&read_tx, &first_block_header.height, &last_block_header.height, None)?;
        BlockHeader::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
        BlockHeader::stream_by_hash(storage.begin_read()?, first_block_header.hash, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_by_timestamp(storage.begin_read()?, first_block_header.timestamp, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_range(storage.begin_read()?, first_block_header.height, last_block_header.height, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_range_by_timestamp(storage.begin_read()?, first_block_header.timestamp, last_block_header.timestamp, None)?.try_collect::<Vec<BlockHeader>>().await?;
    
        let block_header_infos = BlockHeader::table_info(Arc::clone(&storage))?;
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

The same api is accessible through http endpoints at http://127.0.0.1:8000/swagger-ui/.

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
model_v1::block::_store                                             902
model_v1::block::_store_and_commit                                  905
model_v1::block::_store_many                                        919
model_v1::transaction::_store_and_commit                           1292
model_v1::transaction::_store                                      1464
model_v1::transaction::_store_many                                 1516
model_v1::blockheader::_store_many                                 2244
model_v1::utxo::_store_and_commit                                  2321
model_v1::blockheader::_store_and_commit                           2357
model_v1::blockheader::_store                                      2362
model_v1::utxo::_store                                             2440
model_v1::utxo::_store_many                                        2466
model_v1::block::_take                                             3540
model_v1::block::_tail                                             3552
model_v1::asset::_store                                            3739
model_v1::asset::_store_and_commit                                 3831
model_v1::asset::_store_many                                       3894
model_v1::maybevalue::_store                                       4838
model_v1::inputref::_store_many                                    4860
model_v1::inputref::_store                                         4899
model_v1::maybevalue::_store_and_commit                            4908
model_v1::block::_delete_and_commit                                4913
model_v1::maybevalue::_store_many                                  4935
model_v1::inputref::_store_and_commit                              5115
model_v1::utxo::_delete_and_commit                                 6225
model_v1::asset::_delete_and_commit                                6498
model_v1::transaction::_delete_and_commit                          6595
model_v1::blockheader::_delete_and_commit                          6807
model_v1::inputref::_delete_and_commit                             6811
model_v1::block::_get                                              7015
model_v1::block::_first                                            7071
model_v1::block::_last                                             7108
model_v1::maybevalue::_delete_and_commit                           7262
model_v1::block::_get_transactions                                 7463
model_v1::transaction::_tail                                      11569
model_v1::transaction::_take                                      11642
model_v1::transaction::_get_by_hash                               23246
model_v1::transaction::_last                                      23418
model_v1::transaction::_get                                       23423
model_v1::transaction::_first                                     23545
model_v1::transaction::_get_utxos                                 26089
model_v1::block::_range                                           33465
model_v1::block::_stream_range                                    34528
model_v1::transaction::_stream_blocks_by_hash                     37628
model_v1::block::_filter                                          39037
model_v1::utxo::_tail                                             43218
model_v1::utxo::_take                                             43801
model_v1::transaction::_range                                     52044
model_v1::transaction::_stream_range                              52885
model_v1::transaction::_stream_by_hash                            55178
model_v1::utxo::_stream_transactions_by_address                   56738
model_v1::transaction::_filter                                    59626
model_v1::utxo::_get_by_address                                   76431
model_v1::utxo::_get                                              84668
model_v1::utxo::_first                                            85801
model_v1::utxo::_last                                             86585
model_v1::utxo::_range                                            91865
model_v1::utxo::_stream_range                                     94765
model_v1::utxo::_stream_by_address                                95471
model_v1::asset::_stream_utxos_by_name                           100590
model_v1::utxo::_filter                                          114234
model_v1::utxo::_get_assets                                      118172
model_v1::blockheader::_tail                                     131288
model_v1::blockheader::_take                                     133920
model_v1::blockheader::_stream_range_by_duration                 141345
model_v1::blockheader::_stream_range_by_timestamp                142910
model_v1::asset::_tail                                           166837
model_v1::blockheader::_range                                    176612
model_v1::asset::_take                                           181700
model_v1::blockheader::_stream_range                             184506
model_v1::asset::_range                                          203956
model_v1::blockheader::_stream_by_duration                       206165
model_v1::blockheader::_stream_by_hash                           207632
model_v1::blockheader::_stream_by_prev_hash                      210476
model_v1::blockheader::_stream_by_timestamp                      211407
model_v1::blockheader::_range_by_duration                        219958
model_v1::asset::_stream_range                                   220268
model_v1::block::_get_header                                     221905
model_v1::asset::_stream_by_name                                 222909
model_v1::blockheader::_range_by_timestamp                       231778
model_v1::blockheader::_get_by_prev_hash                         241408
model_v1::blockheader::_get_by_hash                              244317
model_v1::blockheader::_get_by_duration                          246602
model_v1::blockheader::_get_by_timestamp                         249049
model_v1::asset::_get_by_name                                    258603
model_v1::blockheader::_filter                                   262013
model_v1::blockheader::_last                                     265670
model_v1::blockheader::_first                                    266251
model_v1::blockheader::_get                                      266332
model_v1::maybevalue::_range                                     313915
model_v1::asset::_filter                                         349945
model_v1::asset::_get                                            355549
model_v1::asset::_first                                          361834
model_v1::maybevalue::_stream_range                              367260
model_v1::asset::_last                                           370964
model_v1::maybevalue::_tail                                      391785
model_v1::utxo::_stream_ids_by_address                           444002
model_v1::maybevalue::_take                                      444182
model_v1::maybevalue::_stream_by_hash                            502364
model_v1::asset::_stream_ids_by_name                             504566
model_v1::utxo::_get_ids_by_address                              547426
model_v1::inputref::_stream_range                                564134
model_v1::maybevalue::_get_by_hash                               615093
model_v1::asset::_get_ids_by_name                                625297
model_v1::utxo::_pk_range                                        663522
model_v1::inputref::_pk_range                                    683074
model_v1::transaction::_pk_range                                 702509
model_v1::asset::_pk_range                                       704518
model_v1::transaction::_get_inputs                               710747
model_v1::transaction::_get_maybe_value                          713572
model_v1::maybevalue::_pk_range                                  722695
model_v1::inputref::_tail                                        749097
model_v1::block::_pk_range                                       751620
model_v1::blockheader::_pk_range                                 780610
model_v1::maybevalue::_get                                       789515
model_v1::blockheader::_stream_heights_by_hash                   816393
model_v1::maybevalue::_filter                                    818090
model_v1::maybevalue::_stream_ids_by_hash                        820385
model_v1::transaction::_stream_ids_by_hash                       825689
model_v1::inputref::_range                                       826665
model_v1::maybevalue::_first                                     843853
model_v1::maybevalue::_last                                      849849
model_v1::blockheader::_stream_heights_by_prev_hash              871832
model_v1::blockheader::_stream_heights_by_duration               885011
model_v1::blockheader::_stream_heights_by_timestamp              913200
model_v1::inputref::_take                                       1003764
model_v1::transaction::_get_ids_by_hash                         1037474
model_v1::blockheader::_get_heights_by_hash                     1097237
model_v1::maybevalue::_get_ids_by_hash                          1135576
model_v1::blockheader::_get_heights_by_prev_hash                1173599
model_v1::blockheader::_get_heights_by_duration                 1216279
model_v1::blockheader::_get_heights_by_timestamp                1253761
model_v1::transaction::_exists                                  1377505
model_v1::inputref::_filter                                     1566760
model_v1::utxo::_exists                                         1573391
model_v1::inputref::_exists                                     1576193
model_v1::inputref::_get                                        1576317
model_v1::maybevalue::_exists                                   1659448
model_v1::block::_exists                                        1681633
model_v1::asset::_exists                                        1761277
model_v1::inputref::_last                                       1764042
model_v1::inputref::_first                                      1766316
model_v1::blockheader::_exists                                  1882601
```
<!-- END_BENCH -->
