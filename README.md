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

Let's say we want to persist and query blockchain data using Redbit, declare annotated Structs `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    pub use redbit::*;
    
    #[root_key] pub struct Height(pub u32);
    
    #[pointer_key(u16)] pub struct BlockPointer(Height);
    #[pointer_key(u16)] pub struct TransactionPointer(BlockPointer);
    #[pointer_key(u8)] pub struct UtxoPointer(TransactionPointer);
    
    // #[column] pub struct Time(pub chrono::DateTime<chrono::Utc>);
    
    #[column("hex")] pub struct Hash(pub [u8; 32]);
    #[column("base64")] pub struct Address(pub Vec<u8>);
    #[column("utf-8")] pub struct AssetName(pub Vec<u8>); // String is supported but this is more efficient
    #[column] pub struct Duration(pub std::time::Duration);
    #[column]
    #[derive(Copy, Hash)]
    pub struct Timestamp(pub u32);
    
    #[column]
    pub struct TempInputRef {
        tx_hash: Hash,
        index: u32,
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
        pub hash: Hash,
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
        pub hash: Hash,
        pub utxos: Vec<Utxo>,
        pub input: Option<InputRef>, // intentionally Option to demonstrate it is possible
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
        #[fk(one2opt)]
        pub id: BlockPointer,
        #[column(index)]
        pub hash: Hash, // just dummy values
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
```
<!-- END_LIB -->

And R/W entire instances efficiently using indexes and dictionaries `examples/utxo/src/demo.rs`:

<!-- BEGIN_MAIN -->
```rust
    use redbit::{AppError, Storage};
    use std::sync::Arc;
    use crate::model_v1::*;
    
    pub async fn with_db(storage: Arc<Storage>) -> () {
        run_with_db(storage).await.unwrap_or_else(|e| eprintln!("{}", e))
    }
    
    async fn run_with_db(storage: Arc<Storage>) -> Result<(), AppError> {
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
        Transaction::get_input(&read_tx, &first_transaction.id)?;
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

### Tracing 

Tracing using [tokio console](https://github.com/tokio-rs/console)

``` 
cargo install --locked tokio-console
RUSTFLAGS="--cfg tokio_unstable" cargo run --features tracing
tokio-console
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
function                                           ops/s
-------------------------------------------------------------
model_v1::block::_store_many                         983
model_v1::block::_store                             1002
model_v1::block::_store_and_commit                  1013
model_v1::transaction::_store_many                  1640
model_v1::transaction::_store_and_commit            1646
model_v1::transaction::_store                       1650
model_v1::utxo::_store_many                         2427
model_v1::utxo::_store_and_commit                   2435
model_v1::utxo::_store                              2462
model_v1::blockheader::_store_many                  2758
model_v1::blockheader::_store_and_commit            2767
model_v1::blockheader::_store                       2808
model_v1::asset::_store                             2940
model_v1::asset::_store_many                        3390
model_v1::inputref::_store_many                     3662
model_v1::inputref::_store_and_commit               3695
model_v1::block::_take                              3698
model_v1::block::_tail                              3722
model_v1::inputref::_store                          3745
model_v1::asset::_store_and_commit                  3841
model_v1::block::_delete_and_commit                 4258
model_v1::utxo::_delete_and_commit                  4601
model_v1::asset::_delete_and_commit                 4882
model_v1::inputref::_delete_and_commit              5295
model_v1::blockheader::_delete_and_commit           5332
model_v1::transaction::_delete_and_commit           6729
model_v1::block::_first                             7356
model_v1::block::_last                              7413
model_v1::block::_get                               7430
model_v1::block::_get_transactions                  7735
model_v1::transaction::_tail                       12038
model_v1::transaction::_take                       12081
model_v1::transaction::_get_by_hash                23910
model_v1::transaction::_last                       24305
model_v1::transaction::_first                      24367
model_v1::transaction::_get                        24417
model_v1::transaction::_get_utxos                  26374
model_v1::block::_range                            36650
model_v1::block::_stream_range                     38210
model_v1::transaction::_stream_blocks_by_hash      41747
model_v1::block::_filter                           42637
model_v1::utxo::_tail                              44163
model_v1::utxo::_take                              44352
model_v1::transaction::_range                      56370
model_v1::transaction::_stream_range               57274
model_v1::transaction::_stream_by_hash             60073
model_v1::utxo::_stream_transactions_by_address      63654
model_v1::transaction::_filter                     66045
model_v1::utxo::_get_by_address                    79527
model_v1::utxo::_get                               89027
model_v1::utxo::_first                             90007
model_v1::utxo::_last                              90664
model_v1::utxo::_range                             96288
model_v1::utxo::_stream_range                      96688
model_v1::utxo::_stream_by_address                 96883
model_v1::asset::_stream_utxos_by_name            101492
model_v1::utxo::_filter                           121082
model_v1::utxo::_get_assets                       122483
model_v1::blockheader::_stream_range_by_timestamp     155929
model_v1::blockheader::_stream_range_by_duration     157983
model_v1::blockheader::_take                      159920
model_v1::blockheader::_tail                      161617
model_v1::asset::_tail                            170847
model_v1::asset::_take                            180527
model_v1::blockheader::_range                     198348
model_v1::asset::_range                           206659
model_v1::blockheader::_stream_range              212370
model_v1::asset::_stream_by_name                  217118
model_v1::asset::_stream_range                    217149
model_v1::blockheader::_stream_by_hash            244396
model_v1::blockheader::_stream_by_duration        248093
model_v1::blockheader::_stream_by_timestamp       249128
model_v1::asset::_get_by_name                     261560
model_v1::blockheader::_range_by_duration         266788
model_v1::blockheader::_range_by_timestamp        270501
model_v1::block::_get_header                      279495
model_v1::blockheader::_get_by_hash               284085
model_v1::blockheader::_get_by_timestamp          286194
model_v1::blockheader::_get_by_duration           287166
model_v1::blockheader::_get                       319035
model_v1::blockheader::_filter                    322681
model_v1::inputref::_range                        323036
model_v1::blockheader::_last                      324672
model_v1::blockheader::_first                     327375
model_v1::inputref::_stream_range                 354234
model_v1::asset::_filter                          360380
model_v1::asset::_get                             364884
model_v1::asset::_last                            365625
model_v1::asset::_first                           370449
model_v1::inputref::_tail                         392824
model_v1::utxo::_stream_ids_by_address            425170
model_v1::inputref::_take                         427813
model_v1::asset::_stream_ids_by_name              484039
model_v1::inputref::_stream_by_hash               501885
model_v1::utxo::_get_ids_by_address               555139
model_v1::asset::_get_ids_by_name                 627058
model_v1::inputref::_get_by_hash                  657436
model_v1::utxo::_pk_range                         692977
model_v1::asset::_pk_range                        705154
model_v1::transaction::_pk_range                  711207
model_v1::block::_pk_range                        726201
model_v1::transaction::_get_input                 734139
model_v1::inputref::_pk_range                     754210
model_v1::transaction::_stream_ids_by_hash        763726
model_v1::blockheader::_pk_range                  807800
model_v1::inputref::_get                          820829
model_v1::inputref::_filter                       823005
model_v1::inputref::_stream_ids_by_hash           833181
model_v1::blockheader::_stream_heights_by_hash     835429
model_v1::inputref::_first                        847235
model_v1::inputref::_last                         856399
model_v1::blockheader::_stream_heights_by_duration     869270
model_v1::blockheader::_stream_heights_by_timestamp     884776
model_v1::transaction::_get_ids_by_hash           975772
model_v1::blockheader::_get_heights_by_hash      1115860
model_v1::inputref::_get_ids_by_hash             1154974
model_v1::blockheader::_get_heights_by_duration    1198409
model_v1::blockheader::_get_heights_by_timestamp    1255382
model_v1::transaction::_exists                   1558336
model_v1::block::_exists                         1656424
model_v1::utxo::_exists                          1665751
model_v1::asset::_exists                         1866995
model_v1::inputref::_exists                      1879346
model_v1::blockheader::_exists                   2097535
```
<!-- END_BENCH -->
