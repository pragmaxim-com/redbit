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
model_v1::block::_store                                             903
model_v1::block::_store_and_commit                                  905
model_v1::block::_store_many                                        926
model_v1::transaction::_store_many                                 1375
model_v1::transaction::_store_and_commit                           1531
model_v1::transaction::_store                                      1533
model_v1::utxo::_store                                             2141
model_v1::blockheader::_store                                      2149
model_v1::blockheader::_store_and_commit                           2154
model_v1::blockheader::_store_many                                 2190
model_v1::utxo::_store_and_commit                                  2299
model_v1::utxo::_store_many                                        2399
model_v1::block::_take                                             3541
model_v1::block::_tail                                             3565
model_v1::asset::_store_and_commit                                 3821
model_v1::asset::_store                                            3827
model_v1::asset::_store_many                                       3927
model_v1::maybevalue::_store                                       4838
model_v1::maybevalue::_store_and_commit                            4875
model_v1::maybevalue::_store_many                                  4926
model_v1::inputref::_store_many                                    6091
model_v1::inputref::_store_and_commit                              6236
model_v1::inputref::_store                                         6251
model_v1::block::_delete_and_commit                                6600
model_v1::block::_get                                              7097
model_v1::block::_last                                             7101
model_v1::block::_first                                            7125
model_v1::block::_get_transactions                                 7408
model_v1::transaction::_delete_and_commit                          8459
model_v1::utxo::_delete_and_commit                                 9265
model_v1::asset::_delete_and_commit                               10148
model_v1::blockheader::_delete_and_commit                         10386
model_v1::maybevalue::_delete_and_commit                          11354
model_v1::inputref::_delete_and_commit                            11557
model_v1::transaction::_tail                                      11668
model_v1::transaction::_take                                      11807
model_v1::transaction::_get_by_hash                               23392
model_v1::transaction::_last                                      23578
model_v1::transaction::_get                                       23691
model_v1::transaction::_first                                     23770
model_v1::transaction::_get_utxos                                 26516
model_v1::block::_range                                           33109
model_v1::block::_stream_range                                    34362
model_v1::transaction::_stream_blocks_by_hash                     37551
model_v1::block::_filter                                          38546
model_v1::utxo::_tail                                             43514
model_v1::utxo::_take                                             43846
model_v1::transaction::_range                                     51567
model_v1::transaction::_stream_range                              52918
model_v1::transaction::_stream_by_hash                            55121
model_v1::utxo::_stream_transactions_by_address                   58226
model_v1::transaction::_filter                                    60087
model_v1::utxo::_get_by_address                                   78311
model_v1::utxo::_get                                              87138
model_v1::utxo::_first                                            87516
model_v1::utxo::_last                                             88447
model_v1::utxo::_range                                            93315
model_v1::utxo::_stream_by_address                                94126
model_v1::utxo::_stream_range                                     94739
model_v1::asset::_stream_utxos_by_name                           101970
model_v1::utxo::_filter                                          115068
model_v1::utxo::_get_assets                                      120938
model_v1::blockheader::_tail                                     130967
model_v1::blockheader::_take                                     132868
model_v1::blockheader::_stream_range_by_duration                 142040
model_v1::blockheader::_stream_range_by_timestamp                142890
model_v1::asset::_tail                                           173731
model_v1::blockheader::_range                                    177371
model_v1::asset::_take                                           183251
model_v1::blockheader::_stream_range                             185646
model_v1::blockheader::_stream_by_prev_hash                      206859
model_v1::blockheader::_stream_by_hash                           208471
model_v1::blockheader::_stream_by_duration                       208581
model_v1::asset::_range                                          208890
model_v1::blockheader::_stream_by_timestamp                      209095
model_v1::asset::_stream_range                                   218531
model_v1::asset::_stream_by_name                                 221113
model_v1::block::_get_header                                     225210
model_v1::blockheader::_range_by_timestamp                       227936
model_v1::blockheader::_range_by_duration                        228803
model_v1::blockheader::_get_by_hash                              243300
model_v1::blockheader::_get_by_prev_hash                         243858
model_v1::blockheader::_get_by_duration                          245130
model_v1::blockheader::_get_by_timestamp                         246993
model_v1::blockheader::_get                                      260704
model_v1::blockheader::_filter                                   262035
model_v1::blockheader::_first                                    266315
model_v1::asset::_get_by_name                                    267046
model_v1::blockheader::_last                                     267846
model_v1::maybevalue::_range                                     309694
model_v1::maybevalue::_stream_range                              351385
model_v1::asset::_filter                                         356939
model_v1::asset::_get                                            358532
model_v1::maybevalue::_tail                                      375475
model_v1::asset::_last                                           375909
model_v1::asset::_first                                          376885
model_v1::maybevalue::_take                                      424128
model_v1::utxo::_stream_ids_by_address                           441876
model_v1::asset::_stream_ids_by_name                             489368
model_v1::maybevalue::_stream_by_hash                            501210
model_v1::utxo::_get_ids_by_address                              562357
model_v1::inputref::_stream_range                                587340
model_v1::asset::_get_ids_by_name                                621315
model_v1::maybevalue::_get_by_hash                               657026
model_v1::utxo::_pk_range                                        695318
model_v1::transaction::_pk_range                                 700064
model_v1::transaction::_get_maybe_value                          701976
model_v1::inputref::_pk_range                                    702183
model_v1::asset::_pk_range                                       719575
model_v1::maybevalue::_pk_range                                  722282
model_v1::transaction::_get_inputs                               730423
model_v1::block::_pk_range                                       750013
model_v1::inputref::_tail                                        768882
model_v1::maybevalue::_filter                                    776229
model_v1::maybevalue::_get                                       777103
model_v1::blockheader::_pk_range                                 783000
model_v1::maybevalue::_first                                     794932
model_v1::maybevalue::_last                                      803749
model_v1::transaction::_stream_ids_by_hash                       817595
model_v1::inputref::_range                                       837051
model_v1::blockheader::_stream_heights_by_hash                   848112
model_v1::maybevalue::_stream_ids_by_hash                        852021
model_v1::blockheader::_stream_heights_by_prev_hash              859749
model_v1::blockheader::_stream_heights_by_duration               882924
model_v1::blockheader::_stream_heights_by_timestamp              889205
model_v1::inputref::_take                                        984533
model_v1::transaction::_get_ids_by_hash                         1057205
model_v1::maybevalue::_get_ids_by_hash                          1135551
model_v1::blockheader::_get_heights_by_hash                     1164483
model_v1::blockheader::_get_heights_by_timestamp                1207132
model_v1::blockheader::_get_heights_by_prev_hash                1209731
model_v1::blockheader::_get_heights_by_duration                 1216427
model_v1::transaction::_exists                                  1347473
model_v1::inputref::_filter                                     1516300
model_v1::inputref::_exists                                     1518488
model_v1::inputref::_get                                        1543615
model_v1::maybevalue::_exists                                   1570352
model_v1::utxo::_exists                                         1572995
model_v1::inputref::_last                                       1674313
model_v1::block::_exists                                        1675014
model_v1::inputref::_first                                      1687109
model_v1::asset::_exists                                        1716650
model_v1::blockheader::_exists                                  1790126
```
<!-- END_BENCH -->
