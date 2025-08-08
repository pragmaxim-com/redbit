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
✅ Optional dictionaries for low cardinality fields \
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
cargo test       # to let all the self-generated tests run (including http layer)
cargo bench      # to run benchmarks
cargo run        # to run the demo example and start the server
```

Check the [redbit-ui](http://github.com/pragmaxim-com/redbit-ui) for frontend dev.

The utxo example has close to 500 frontend/backend derived tests and 130 benchmarks, so that if any redbit app derived from the definition compiles,
it is transparent, well tested and benched already.

Let's say we want to persist and query blockchain data using Redbit, declare annotated Structs `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    #![feature(test)]
    extern crate test;
    
    pub mod data;
    pub mod run;
    pub mod routes;
    
    pub use data::*;
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
        #[column(dictionary)]
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
        #[column(dictionary)]
        pub name: AssetName,
    }
```
<!-- END_LIB -->

And R/W entire instances efficiently using indexes and dictionaries `examples/utxo/src/demo.rs`:

<!-- BEGIN_MAIN -->
```rust
    use crate::*;
    use redb::Database;
    use redbit::AppError;
    use std::sync::Arc;
    
    pub async fn with_db(db: Arc<Database>) -> Result<(), AppError> {
        let blocks = Block::sample_many(2);
    
        println!("Persisting blocks:");
        let write_tx = db.begin_write()?;
        Block::store_many(&write_tx, &blocks)?;
        write_tx.commit()?;
    
        let read_tx = db.begin_read()?;
    
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
        Block::stream_range(db.begin_read()?, first_block.height, last_block.height, None)?.try_collect::<Vec<Block>>().await?;
    
        let block_infos = Block::table_info(&db)?;
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
        BlockHeader::stream_by_hash(db.begin_read()?, first_block_header.hash, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_by_timestamp(db.begin_read()?, first_block_header.timestamp, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_range(db.begin_read()?, first_block_header.height, last_block_header.height, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_range_by_timestamp(db.begin_read()?, first_block_header.timestamp, last_block_header.timestamp, None)?.try_collect::<Vec<BlockHeader>>().await?;
    
        let block_header_infos = BlockHeader::table_info(&db)?;
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
        Transaction::stream_by_hash(db.begin_read()?, first_transaction.hash.clone(), None)?.try_collect::<Vec<Transaction>>().await?;
        Transaction::stream_range(db.begin_read()?, first_transaction.id, last_transaction.id, None)?.try_collect::<Vec<Transaction>>().await?;
    
        let transaction_infos = Transaction::table_info(&db)?;
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
        Utxo::stream_range(db.begin_read()?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_address(db.begin_read()?, first_utxo.address.clone(), None)?.try_collect::<Vec<Utxo>>().await?;
        // even streaming parents is possible
        Utxo::stream_transactions_by_address(db.begin_read()?, first_utxo.address, None)?.try_collect::<Vec<Transaction>>().await?;
    
        let utxo_infos = Utxo::table_info(&db)?;
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
        Asset::stream_by_name(db.begin_read()?, first_asset.name.clone(), None)?.try_collect::<Vec<Asset>>().await?;
        Asset::stream_range(db.begin_read()?, first_asset.id, last_asset.id, None)?.try_collect::<Vec<Asset>>().await?;
        // even streaming parents is possible
        Asset::stream_utxos_by_name(db.begin_read()?, first_asset.name, None)?.try_collect::<Vec<Utxo>>().await?;
    
        let asset_infos = Asset::table_info(&db)?;
        println!("
Asset persisted with tables :");
        for info in asset_infos {
            println!("{}", serde_json::to_string_pretty(&info).unwrap());
        }
    
    
        println!("
Deleting blocks:");
        for block in blocks.iter() {
            Block::delete_and_commit(&db, &block.height)?;
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
block::_store_many                                  1234
transaction::_store                                 1639
transaction::_store_and_commit                      1669
transaction::_store_many                            1672
block::_store_and_commit                            2037
block::_store                                       2073
utxo::_store                                        2615
utxo::_store_and_commit                             2630
utxo::_store_many                                   2685
blockheader::_store_and_commit                      2916
blockheader::_store_many                            3085
blockheader::_store                                 3126
asset::_store_many                                  3837
block::_take                                        3837
block::_tail                                        3849
asset::_store                                       3862
asset::_store_and_commit                            3881
inputref::_store_many                               4663
inputref::_store_and_commit                         4830
inputref::_store                                    5448
utxo::_delete_and_commit                            5717
block::_delete_and_commit                           5761
transaction::_delete_and_commit                     6358
asset::_delete_and_commit                           6856
block::_last                                        7671
block::_get                                         7675
block::_first                                       7697
blockheader::_delete_and_commit                     7819
block::_get_transactions                            7950
inputref::_delete_and_commit                        9759
transaction::_take                                 12471
transaction::_tail                                 12524
transaction::_get_by_hash                          24727
transaction::_last                                 24889
transaction::_get                                  24915
transaction::_first                                24929
transaction::_get_utxos                            27241
block::_range                                      38794
block::_stream_range                               39886
transaction::_stream_blocks_by_hash                43537
block::_filter                                     44620
utxo::_tail                                        44656
utxo::_take                                        44873
transaction::_range                                58405
transaction::_stream_range                         59722
transaction::_stream_by_hash                       62458
utxo::_stream_transactions_by_address              64176
transaction::_filter                               67754
utxo::_get_by_address                              80827
utxo::_get                                         89296
utxo::_first                                       89497
utxo::_last                                        89852
utxo::_range                                       95384
utxo::_stream_by_address                           98417
utxo::_stream_range                                98772
asset::_stream_utxos_by_name                      103665
utxo::_filter                                     118981
utxo::_get_assets                                 120062
blockheader::_stream_range_by_timestamp           154033
blockheader::_stream_range_by_duration            154376
blockheader::_tail                                160300
blockheader::_take                                163102
asset::_tail                                      173323
asset::_take                                      184083
blockheader::_range                               201948
asset::_range                                     206106
blockheader::_stream_range                        213531
asset::_stream_by_name                            219419
asset::_stream_range                              221220
blockheader::_stream_by_hash                      246089
blockheader::_stream_by_duration                  249982
blockheader::_stream_by_timestamp                 254363
asset::_get_by_name                               256687
blockheader::_range_by_duration                   262868
blockheader::_range_by_timestamp                  264772
blockheader::_get_by_hash                         286215
blockheader::_get_by_timestamp                    287812
blockheader::_get_by_duration                     289229
block::_get_header                                303193
blockheader::_filter                              320044
blockheader::_last                                321416
blockheader::_get                                 324020
blockheader::_first                               325008
inputref::_range                                  327340
asset::_filter                                    361342
asset::_get                                       362841
inputref::_stream_range                           368846
asset::_first                                     370157
asset::_last                                      371950
inputref::_tail                                   411100
inputref::_take                                   453850
utxo::_stream_ids_by_address                      472286
asset::_stream_ids_by_name                        492492
inputref::_stream_by_hash                         513073
utxo::_get_ids_by_address                         572616
asset::_get_ids_by_name                           604233
utxo::_pk_range                                   683873
asset::_pk_range                                  685824
inputref::_get_by_hash                            689855
transaction::_pk_range                            741455
inputref::_pk_range                               748761
block::_pk_range                                  752191
transaction::_get_input                           785811
blockheader::_pk_range                            817648
inputref::_filter                                 829270
inputref::_get                                    848407
inputref::_last                                   859956
transaction::_stream_ids_by_hash                  870944
inputref::_first                                  875802
blockheader::_stream_heights_by_hash              876140
blockheader::_stream_heights_by_duration          879941
blockheader::_stream_heights_by_timestamp         888676
inputref::_stream_ids_by_hash                     923642
transaction::_get_ids_by_hash                    1148264
blockheader::_get_heights_by_duration            1249485
blockheader::_get_heights_by_timestamp           1269970
blockheader::_get_heights_by_hash                1272168
inputref::_get_ids_by_hash                       1315755
block::_exists                                   1692133
utxo::_exists                                    1720016
asset::_exists                                   1814619
transaction::_exists                             1894944
inputref::_exists                                1909600
blockheader::_exists                             2143853
```
<!-- END_BENCH -->
