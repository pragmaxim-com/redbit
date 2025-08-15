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
✅ All pointers and most column types implement `Copy` => minimal cloning \
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
model_v1::block::_store                                             900
model_v1::block::_store_and_commit                                  900
model_v1::block::_store_many                                        917
model_v1::transaction::_store                                      1360
model_v1::transaction::_store_and_commit                           1444
model_v1::transaction::_store_many                                 1515
model_v1::blockheader::_store                                      2400
model_v1::blockheader::_store_and_commit                           2425
model_v1::blockheader::_store_many                                 2436
model_v1::utxo::_store_many                                        2459
model_v1::utxo::_store                                             2461
model_v1::utxo::_store_and_commit                                  2474
model_v1::block::_take                                             3466
model_v1::block::_tail                                             3503
model_v1::asset::_store_and_commit                                 3507
model_v1::asset::_store_many                                       3613
model_v1::asset::_store                                            3637
model_v1::maybevalue::_store_many                                  3659
model_v1::maybevalue::_store                                       3733
model_v1::maybevalue::_store_and_commit                            3892
model_v1::inputref::_store_and_commit                              4104
model_v1::inputref::_store                                         4318
model_v1::block::_delete_and_commit                                4353
model_v1::inputref::_store_many                                    4412
model_v1::transaction::_delete_and_commit                          4575
model_v1::maybevalue::_delete_and_commit                           5145
model_v1::utxo::_delete_and_commit                                 5306
model_v1::blockheader::_delete_and_commit                          5916
model_v1::asset::_delete_and_commit                                5973
model_v1::inputref::_delete_and_commit                             6052
model_v1::block::_get                                              7027
model_v1::block::_first                                            7085
model_v1::block::_last                                             7094
model_v1::block::_get_transactions                                 7343
model_v1::transaction::_tail                                      11285
model_v1::transaction::_take                                      11531
model_v1::transaction::_get_by_hash                               22879
model_v1::transaction::_get                                       23214
model_v1::transaction::_first                                     23222
model_v1::transaction::_last                                      23244
model_v1::transaction::_get_utxos                                 26192
model_v1::block::_range                                           33886
model_v1::block::_stream_range                                    34779
model_v1::transaction::_stream_blocks_by_hash                     38079
model_v1::block::_filter                                          39255
model_v1::utxo::_tail                                             43315
model_v1::utxo::_take                                             43615
model_v1::transaction::_range                                     51857
model_v1::transaction::_stream_range                              52688
model_v1::transaction::_stream_by_hash                            55251
model_v1::utxo::_stream_transactions_by_address                   58676
model_v1::transaction::_filter                                    60024
model_v1::utxo::_get_by_address                                   77178
model_v1::utxo::_get                                              84475
model_v1::utxo::_first                                            85204
model_v1::utxo::_last                                             86859
model_v1::utxo::_range                                            91979
model_v1::utxo::_stream_range                                     94863
model_v1::utxo::_stream_by_address                                95682
model_v1::asset::_stream_utxos_by_name                           101893
model_v1::utxo::_filter                                          114285
model_v1::utxo::_get_assets                                      118366
model_v1::blockheader::_tail                                     135252
model_v1::blockheader::_take                                     135923
model_v1::blockheader::_stream_range_by_timestamp                141261
model_v1::blockheader::_stream_range_by_duration                 141443
model_v1::asset::_tail                                           169300
model_v1::blockheader::_range                                    178295
model_v1::asset::_take                                           179121
model_v1::blockheader::_stream_range                             187239
model_v1::asset::_range                                          202007
model_v1::blockheader::_stream_by_hash                           211017
model_v1::blockheader::_stream_by_prev_hash                      212314
model_v1::blockheader::_stream_by_duration                       214328
model_v1::asset::_stream_range                                   215542
model_v1::blockheader::_stream_by_timestamp                      215584
model_v1::asset::_stream_by_name                                 217445
model_v1::block::_get_header                                     227090
model_v1::blockheader::_range_by_duration                        228658
model_v1::blockheader::_range_by_timestamp                       231088
model_v1::blockheader::_get_by_hash                              243740
model_v1::blockheader::_get_by_prev_hash                         245455
model_v1::blockheader::_get_by_duration                          247636
model_v1::blockheader::_get_by_timestamp                         249028
model_v1::asset::_get_by_name                                    261101
model_v1::blockheader::_filter                                   264479
model_v1::blockheader::_first                                    272114
model_v1::blockheader::_get                                      272247
model_v1::blockheader::_last                                     273863
model_v1::maybevalue::_range                                     307760
model_v1::asset::_filter                                         353474
model_v1::asset::_get                                            355443
model_v1::asset::_last                                           357957
model_v1::maybevalue::_stream_range                              358622
model_v1::asset::_first                                          363158
model_v1::maybevalue::_tail                                      387977
model_v1::maybevalue::_take                                      428201
model_v1::utxo::_stream_ids_by_address                           439101
model_v1::maybevalue::_stream_by_hash                            484552
model_v1::asset::_stream_ids_by_name                             485812
model_v1::inputref::_stream_range                                538912
model_v1::utxo::_get_ids_by_address                              546200
model_v1::asset::_get_ids_by_name                                595813
model_v1::maybevalue::_get_by_hash                               620347
model_v1::asset::_pk_range                                       663178
model_v1::utxo::_pk_range                                        665040
model_v1::inputref::_pk_range                                    666831
model_v1::transaction::_pk_range                                 687758
model_v1::transaction::_get_inputs                               714694
model_v1::transaction::_get_maybe_value                          729613
model_v1::inputref::_tail                                        730684
model_v1::block::_pk_range                                       768002
model_v1::maybevalue::_pk_range                                  769497
model_v1::maybevalue::_stream_ids_by_hash                        787141
model_v1::maybevalue::_filter                                    788339
model_v1::maybevalue::_get                                       794344
model_v1::transaction::_stream_ids_by_hash                       804855
model_v1::blockheader::_pk_range                                 811491
model_v1::blockheader::_stream_heights_by_hash                   818197
model_v1::inputref::_range                                       829476
model_v1::maybevalue::_last                                      836267
model_v1::maybevalue::_first                                     841432
model_v1::blockheader::_stream_heights_by_prev_hash              842105
model_v1::blockheader::_stream_heights_by_duration               856025
model_v1::blockheader::_stream_heights_by_timestamp              865089
model_v1::inputref::_take                                       1009540
model_v1::transaction::_get_ids_by_hash                         1035207
model_v1::maybevalue::_get_ids_by_hash                          1068913
model_v1::blockheader::_get_heights_by_hash                     1097647
model_v1::blockheader::_get_heights_by_prev_hash                1145436
model_v1::blockheader::_get_heights_by_duration                 1169905
model_v1::blockheader::_get_heights_by_timestamp                1251314
model_v1::transaction::_exists                                  1416531
model_v1::inputref::_filter                                     1474448
model_v1::inputref::_exists                                     1489225
model_v1::utxo::_exists                                         1584585
model_v1::inputref::_get                                        1611915
model_v1::maybevalue::_exists                                   1698658
model_v1::block::_exists                                        1747183
model_v1::asset::_exists                                        1803622
model_v1::inputref::_last                                       1803817
model_v1::inputref::_first                                      1804631
model_v1::blockheader::_exists                                  1948027
```
<!-- END_BENCH -->
