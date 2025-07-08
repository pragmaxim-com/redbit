Built for blazing fast persistence of terra bytes of structured data on a single machine
while offering rich querying capabilities, e.g. bitcoin/blockchain data. Blockchains need rich and often
analytical queries which is done through explorers because indexing speed of even embedded/in-process (not through socket) 
analytical db like [DuckDB](https://duckdb.org/) right on the node would be an order of magnitude slower than a hand-made solution on top of 
[Redb](https://github.com/cberner/redb) or [RocksDb](https://rocksdb.org/).

Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from
[Redb](https://github.com/cberner/redb) using secondary indexes and dictionaries, served by [axum](https://github.com/tokio-rs/axum)
through auto-generated REST API. It maximizes R/W speed while minimizing data size using hierarchical data structures of smart pointers.

### Major Out-of-the-Box Features

✅ Querying and ranging by secondary index \
✅ Optional dictionaries for low cardinality fields \
✅ One-to-One / One-to-Option / One-to-Many entities with cascade read/write/delete \
✅ All goodies including intuitive data ordering without writing custom codecs \
✅ SSE streaming api with efficient querying (ie. get txs or utxos for really HOT address) \
✅ query contraints : `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `in` with logical `AND`
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
✅ column types : `String`, `Int`, `Vec<u8>`, `[u8; N]`, `bool`, `uuid::Uuid`, `chrono::DateTime`, `std::time::Duration` \
✅ column encodings of binary columns : `hex`, `base64` \
✅ all types have binary (db) and human-readable (http) serde support \
✅ Macro derived http rest API at http://127.0.0.1:8000/swagger-ui/ with examples \
✅ Macro derived unit tests and integration tests on axum test server

### Limitations

❌ root key must be newtype struct with numeric inner type (that's part of the design decision to achieve fast indexing of even whole bitcoin)

```
cargo run --package utxo                # to run the demo example
cargo test --package utxo               # to let all the self-generated tests run (including http layer)
```

Let's say we want to persist and query blockchain data using Redbit, declare annotated Structs `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    pub mod data;
    pub mod demo;
    pub mod routes;
    
    pub use data::*;
    pub use redbit::*;
    
    #[root_key] pub struct Height(pub u32);
    
    #[pointer_key(u16)] pub struct TxPointer(Height);
    #[pointer_key(u16)] pub struct UtxoPointer(TxPointer);
    #[pointer_key(u16)] pub struct InputPointer(TxPointer);
    #[pointer_key(u8)] pub struct AssetPointer(UtxoPointer);
    
    #[column] pub struct Hash(pub String);
    #[column] pub struct PolicyId(pub String);
    #[column("base64")] pub struct Address(pub [u8; 32]);
    #[column("hex")] pub struct Datum(pub Vec<u8>);
    #[column] pub struct AssetName(pub String);
    #[column] pub struct Time(pub chrono::DateTime<chrono::Utc>);
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
        pub id: Height,
        pub header: BlockHeader,
        pub transactions: Vec<Transaction>,
        #[column(transient)]
        pub weight: u32,
    }
    
    #[entity]
    pub struct BlockHeader {
        #[fk(one2one)]
        pub id: Height,
        #[column(index)]
        pub hash: Hash,
        #[column(range)]
        pub timestamp: Timestamp,
        #[column(range)]
        pub time: Time,
        #[column]
        pub duration: Duration,
        #[column(index)]
        pub merkle_root: Hash,
        #[column]
        pub nonce: u64,
    }
    
    #[entity]
    pub struct Transaction {
        #[fk(one2many)]
        pub id: TxPointer,
        #[column(index)]
        pub hash: Hash,
        pub utxos: Vec<Utxo>,
        #[column(transient)]
        pub transient_inputs: Vec<TempInputRef>,
    }
    
    #[entity]
    pub struct Utxo {
        #[fk(one2many)]
        pub id: UtxoPointer,
        #[column]
        pub amount: u64,
        #[column(index)]
        pub datum: Datum,
        #[column(dictionary)]
        pub address: Address,
        pub assets: Vec<Asset>,
        pub tree: Option<Tree>,
    }
    
    #[entity]
    pub struct Tree {
        #[fk(one2opt)]
        pub id: UtxoPointer,
        #[column(index)]
        pub hash: Hash,
    }
    
    #[entity]
    pub struct Asset {
        #[fk(one2many)]
        pub id: AssetPointer,
        #[column]
        pub amount: u64,
        #[column(dictionary)]
        pub name: AssetName,
        #[column(dictionary)]
        pub policy_id: PolicyId,
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
    
    pub async fn run(db: Arc<Database>) -> Result<(), AppError> {
        let blocks = Block::sample_many(2);
    
        println!("Persisting blocks:");
        let write_tx = db.begin_write()?;
        Block::store_many(&write_tx, &blocks)?;
        write_tx.commit()?;
    
        let read_tx = db.begin_read()?;
    
        println!("Querying blocks:");
        let first_block = Block::first(&read_tx)?.unwrap();
        let last_block = Block::last(&read_tx)?.unwrap();
    
        Block::take(&read_tx, 100)?;
        Block::get(&read_tx, &first_block.id)?;
        Block::range(&read_tx, &first_block.id, &last_block.id, None)?;
        Block::get_transactions(&read_tx, &first_block.id)?;
        Block::get_header(&read_tx, &first_block.id)?;
        Block::exists(&read_tx, &first_block.id)?;
        Block::first(&read_tx)?;
        Block::last(&read_tx)?;
        Block::stream_range(db.begin_read()?, first_block.id, last_block.id, None)?.try_collect::<Vec<Block>>().await?;
    
        println!("Querying block headers:");
        let first_block_header = BlockHeader::first(&read_tx)?.unwrap();
        let last_block_header = BlockHeader::last(&read_tx)?.unwrap();
    
        BlockHeader::get_by_hash(&read_tx, &first_block_header.hash)?;
        BlockHeader::get_by_timestamp(&read_tx, &first_block_header.timestamp)?;
        BlockHeader::get_by_merkle_root(&read_tx, &first_block_header.merkle_root)?;
        BlockHeader::take(&read_tx, 100)?;
        BlockHeader::get(&read_tx, &first_block_header.id)?;
        BlockHeader::range(&read_tx, &first_block_header.id, &last_block_header.id, None)?;
        BlockHeader::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
        BlockHeader::stream_by_hash(db.begin_read()?, first_block_header.hash, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_by_timestamp(db.begin_read()?, first_block_header.timestamp, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_by_merkle_root(db.begin_read()?, first_block_header.merkle_root, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_range(db.begin_read()?, first_block_header.id, last_block_header.id, None)?.try_collect::<Vec<BlockHeader>>().await?;
        BlockHeader::stream_range_by_timestamp(db.begin_read()?, first_block_header.timestamp, last_block_header.timestamp, None)?.try_collect::<Vec<BlockHeader>>().await?;
    
        println!("Querying transactions:");
        let first_transaction = Transaction::first(&read_tx)?.unwrap();
        let last_transaction = Transaction::last(&read_tx)?.unwrap();
    
        Transaction::get_ids_by_hash(&read_tx, &first_transaction.hash)?;
        Transaction::get_by_hash(&read_tx, &first_transaction.hash)?;
        Transaction::take(&read_tx, 100)?;
        Transaction::get(&read_tx, &first_transaction.id)?;
        Transaction::range(&read_tx, &first_transaction.id, &last_transaction.id, None)?;
        Transaction::get_utxos(&read_tx, &first_transaction.id)?;
        Transaction::parent_key(&read_tx, &first_transaction.id)?;
        Transaction::stream_ids_by_hash(&read_tx, &first_transaction.hash)?.try_collect::<Vec<TxPointer>>().await?;
        Transaction::stream_by_hash(db.begin_read()?, first_transaction.hash, None)?.try_collect::<Vec<Transaction>>().await?;
        Transaction::stream_range(db.begin_read()?, first_transaction.id, last_transaction.id, None)?.try_collect::<Vec<Transaction>>().await?;
    
        println!("Querying utxos:");
        let first_utxo = Utxo::first(&read_tx)?.unwrap();
        let last_utxo = Utxo::last(&read_tx)?.unwrap();
    
        Utxo::get_by_address(&read_tx, &first_utxo.address)?;
        Utxo::get_by_datum(&read_tx, &first_utxo.datum)?;
        Utxo::get_ids_by_address(&read_tx, &first_utxo.address)?;
        Utxo::take(&read_tx, 100)?;
        Utxo::get(&read_tx, &first_utxo.id)?;
        Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id, None)?;
        Utxo::get_assets(&read_tx, &first_utxo.id)?;
        Utxo::parent_key(&read_tx, &first_utxo.id)?;
        Utxo::get_tree(&read_tx, &first_utxo.id)?;
        Utxo::stream_ids_by_address(&read_tx, &first_utxo.address)?.try_collect::<Vec<UtxoPointer>>().await?;
        Utxo::stream_range(db.begin_read()?, first_utxo.id, last_utxo.id, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_address(db.begin_read()?, first_utxo.address, None)?.try_collect::<Vec<Utxo>>().await?;
        Utxo::stream_by_datum(db.begin_read()?, first_utxo.datum, None)?.try_collect::<Vec<Utxo>>().await?;
    
        println!("Querying assets:");
        let first_asset = Asset::first(&read_tx)?.unwrap();
        let last_asset = Asset::last(&read_tx)?.unwrap();
    
        Asset::get_by_name(&read_tx, &first_asset.name)?;
        Asset::get_by_policy_id(&read_tx, &first_asset.policy_id)?;
        Asset::get_ids_by_policy_id(&read_tx, &first_asset.policy_id)?;
        Asset::take(&read_tx, 100)?;
        Asset::get(&read_tx, &first_asset.id)?;
        Asset::range(&read_tx, &first_asset.id, &last_asset.id, None)?;
        Asset::parent_key(&read_tx, &first_asset.id)?;
        Asset::stream_ids_by_policy_id(&read_tx, &first_asset.policy_id)?.try_collect::<Vec<AssetPointer>>().await?;
        Asset::stream_by_policy_id(db.begin_read()?, first_asset.policy_id, None)?.try_collect::<Vec<Asset>>().await?;
        Asset::stream_by_name(db.begin_read()?, first_asset.name, None)?.try_collect::<Vec<Asset>>().await?;
        Asset::stream_range(db.begin_read()?, first_asset.id, last_asset.id, None)?.try_collect::<Vec<Asset>>().await?;
    
        println!("Deleting blocks:");
        for block in blocks.iter() {
            Block::delete_and_commit(&db, &block.id)?
        }
        Ok(())
    }
```
<!-- END_MAIN -->

The same api is accessible through http endpoints at http://127.0.0.1:8000/swagger-ui/.

Performance wise, check 🔥[flamegraph](https://rawcdn.githack.com/pragmaxim-com/redbit/refs/heads/master/flamegraph.svg).
The demo example persists data into 30 tables to allow for rich querying.

### ⏱️ Benchmark Summary
The slowest `Block__all` operation in this context equals to reading 3 blocks of 3 transactions of 3 utxos of 3 assets, ie.
the operations reads :
- 3 blocks
- 3 * 3 = 9 transactions
- 3 * 3 * 3 = 27 inputs
- 3 * 3 * 3 = 27 utxos
- 3 * 3 * 3 * 3 = 81 assets

<!-- BEGIN_BENCH -->
```
function                                           ops/s
-------------------------------------------------------------
Block__all                                           692
Transaction__all                                     708
Utxo__all                                            724
Utxo__range                                          755
Transaction__range                                   802
Block__range                                        1038
Asset__all                                          1065
Asset__range                                        1081
Block__get                                          2052
Transaction__get_by_hash                            2083
Block__get_transactions                             2087
Utxo__stream_by_address                             2112
Utxo__get_by_address                                2130
Utxo__stream_by_datum                               2133
Utxo__get_by_datum                                  2148
Asset__stream_by_name                               3131
Asset__get_by_name                                  3132
Asset__stream_by_policy_id                          3146
Asset__get_by_policy_id                             3161
Transaction__get                                    5843
Transaction__get_utxos                              6121
Utxo__get                                          18100
Utxo__get_assets                                   24454
Asset__get                                         66041
BlockHeader__all                                   68104
BlockHeader__stream_range_by_timestamp             89130
BlockHeader__range_by_timestamp                    95484
BlockHeader__range                                 99279
Transaction__stream_by_hash                       164657
BlockHeader__stream_by_hash                       165457
BlockHeader__stream_by_merkle_root                168000
BlockHeader__stream_by_timestamp                  170365
BlockHeader__get_by_merkle_root                   181178
BlockHeader__get_by_hash                          182823
BlockHeader__get_by_timestamp                     189858
BlockHeader__get                                  193592
Block__get_header                                 194217
```
<!-- END_BENCH -->
