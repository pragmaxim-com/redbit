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
    #![feature(test)]
    extern crate test;
    
    pub mod storage;
    pub mod run;
    pub mod routes;
    pub mod model_v1;
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

### ⏱️ Benchmark Summary (results from github servers)

Indexing speed in logs is the **average**, for example, the first ~ 100k **bitcoin** blocks with just one Tx are indexed slowly because 
indexing is optimized for the big blocks.

If node and indexer each uses its own SSD, then the throughput reaches :

 - 2.0GHz & NVMe on PCIe Gen3 : `~ 9 000 Inputs+outputs+assets / s`
 - 3.0GHz & NVMe on PCIe Gen4 : `~ 15 000 Inputs+outputs+assets / s`
 - 4.0GHz & NVMe on PCIe Gen5 : `~ 28 000 Inputs+outputs+assets / s`

In a nutshell, whole bitcoin up to height ~ 0.9M can be indexed in a day on a PCIe Gen5 SSD with 4.0GHz CPU.

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
model_v1::block::_store_many                        1286
model_v1::transaction::_store                       1651
model_v1::transaction::_store_and_commit            1662
model_v1::transaction::_store_many                  1663
model_v1::block::_store_and_commit                  1869
model_v1::block::_store                             1889
model_v1::utxo::_store                              2377
model_v1::utxo::_store_and_commit                   2383
model_v1::utxo::_store_many                         2446
model_v1::blockheader::_store_many                  2819
model_v1::blockheader::_store_and_commit            2972
model_v1::blockheader::_store                       3124
model_v1::block::_take                              3559
model_v1::block::_tail                              3563
model_v1::asset::_store_and_commit                  3774
model_v1::asset::_store_many                        3778
model_v1::asset::_store                             3795
model_v1::inputref::_store                          6067
model_v1::inputref::_store_and_commit               6077
model_v1::inputref::_store_many                     6130
model_v1::block::_delete_and_commit                 6745
model_v1::block::_first                             7183
model_v1::block::_get                               7193
model_v1::block::_last                              7212
model_v1::block::_get_transactions                  7491
model_v1::transaction::_delete_and_commit           8485
model_v1::utxo::_delete_and_commit                  9122
model_v1::blockheader::_delete_and_commit          10010
model_v1::inputref::_delete_and_commit             10385
model_v1::asset::_delete_and_commit                10413
model_v1::transaction::_take                       11515
model_v1::transaction::_tail                       11548
model_v1::transaction::_get_by_hash                22847
model_v1::transaction::_last                       23063
model_v1::transaction::_first                      23192
model_v1::transaction::_get                        23285
model_v1::transaction::_get_utxos                  25116
model_v1::block::_range                            35838
model_v1::block::_stream_range                     36908
model_v1::transaction::_stream_blocks_by_hash      40005
model_v1::block::_filter                           41748
model_v1::utxo::_tail                              43337
model_v1::utxo::_take                              43636
model_v1::transaction::_range                      54637
model_v1::transaction::_stream_range               55693
model_v1::transaction::_stream_by_hash             58082
model_v1::utxo::_stream_transactions_by_address      62331
model_v1::transaction::_filter                     63215
model_v1::utxo::_get_by_address                    76968
model_v1::utxo::_get                               86147
model_v1::utxo::_first                             86401
model_v1::utxo::_last                              86823
model_v1::utxo::_range                             92322
model_v1::utxo::_stream_by_address                 93395
model_v1::utxo::_stream_range                      95804
model_v1::asset::_stream_utxos_by_name             98143
model_v1::utxo::_filter                           116437
model_v1::utxo::_get_assets                       118469
model_v1::blockheader::_stream_range_by_duration     152977
model_v1::blockheader::_tail                      153639
model_v1::blockheader::_stream_range_by_timestamp     155456
model_v1::blockheader::_take                      155975
model_v1::asset::_tail                            168695
model_v1::asset::_take                            177176
model_v1::blockheader::_range                     196999
model_v1::asset::_range                           200996
model_v1::blockheader::_stream_range              210691
model_v1::asset::_stream_by_name                  215428
model_v1::asset::_stream_range                    217596
model_v1::blockheader::_stream_by_hash            238649
model_v1::blockheader::_stream_by_timestamp       243227
model_v1::blockheader::_stream_by_duration        244402
model_v1::asset::_get_by_name                     251751
model_v1::blockheader::_range_by_timestamp        254736
model_v1::blockheader::_range_by_duration         255023
model_v1::block::_get_header                      270050
model_v1::blockheader::_get_by_hash               277118
model_v1::blockheader::_get_by_duration           279544
model_v1::blockheader::_get_by_timestamp          280275
model_v1::blockheader::_filter                    308976
model_v1::blockheader::_get                       310605
model_v1::blockheader::_last                      312146
model_v1::blockheader::_first                     315242
model_v1::inputref::_range                        318084
model_v1::asset::_filter                          349553
model_v1::asset::_get                             353302
model_v1::inputref::_stream_range                 354609
model_v1::asset::_last                            359840
model_v1::asset::_first                           362521
model_v1::inputref::_tail                         392850
model_v1::inputref::_take                         434363
model_v1::utxo::_stream_ids_by_address            437650
model_v1::asset::_stream_ids_by_name              509323
model_v1::inputref::_stream_by_hash               510121
model_v1::utxo::_get_ids_by_address               544410
model_v1::asset::_get_ids_by_name                 621794
model_v1::utxo::_pk_range                         669797
model_v1::inputref::_get_by_hash                  684313
model_v1::asset::_pk_range                        694377
model_v1::block::_pk_range                        712987
model_v1::transaction::_pk_range                  719761
model_v1::transaction::_get_input                 726580
model_v1::inputref::_pk_range                     747787
model_v1::blockheader::_pk_range                  783619
model_v1::transaction::_stream_ids_by_hash        812011
model_v1::inputref::_get                          814584
model_v1::inputref::_filter                       820392
model_v1::inputref::_last                         838328
model_v1::inputref::_first                        840110
model_v1::inputref::_stream_ids_by_hash           860600
model_v1::blockheader::_stream_heights_by_hash     873904
model_v1::blockheader::_stream_heights_by_duration     880964
model_v1::blockheader::_stream_heights_by_timestamp     900998
model_v1::transaction::_get_ids_by_hash          1047724
model_v1::inputref::_get_ids_by_hash             1146657
model_v1::blockheader::_get_heights_by_hash      1222838
model_v1::blockheader::_get_heights_by_duration    1240710
model_v1::blockheader::_get_heights_by_timestamp    1247536
model_v1::transaction::_exists                   1533437
model_v1::utxo::_exists                          1590685
model_v1::block::_exists                         1595482
model_v1::inputref::_exists                      1717652
model_v1::asset::_exists                         1776073
model_v1::blockheader::_exists                   2031860
```
<!-- END_BENCH -->
