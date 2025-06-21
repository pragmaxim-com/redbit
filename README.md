Built for blazing fast persistence of terra bytes of structured data on a single machine
while offering rich querying capabilities, e.g. bitcoin/blockchain data. Blockchains need rich and often
analytical queries which is done through explorers because indexing speed of even embedded/in-process (not through socket) 
analytical db like [DuckDB](https://duckdb.org/) right on the node would be an order of magnitude slower than doing so with 
[Redb](https://github.com/cberner/redb) or [RocksDb](https://rocksdb.org/).

Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from
[Redb](https://github.com/cberner/redb) using secondary indexes and dictionaries, served by [axum](https://github.com/tokio-rs/axum)
through auto-generated REST API.

### Main motivation is a research

- Rust type and macro system and db engines at the byte level
- decentralized persistence options to maximize indexing speed and minimize data size
- meta space : self-tested and self-documented db & http layers of code derived from annotated structs
- maximizing R/W speed while minimizing data size using hierarchical data structures of smart pointers

### Major Out-of-the-Box Features

- ✅ Querying and ranging by secondary index
- ✅ Optional dictionaries for low cardinality fields
- ✅ One-to-One and One-to-Many entities with cascade read/write/delete
- ✅ All goodies including intuitive data ordering without writing custom codecs
- ✅ Macro derived http rest API at http://127.0.0.1:8000/swagger-ui/
- ✅ Macro derived unit tests and integration tests on axum test server

```
cargo run --package utxo                # to run the demo example
cargo test --package utxo               # to let all the self-generated tests run (including http layer)
```

Let's say we want to persist and query blockchain data using Redbit, declare annotated Structs `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    pub mod data;
    pub mod demo;
    
    pub use data::*;
    pub use redbit::*;
    
    #[root_key] pub struct Height(pub u32);
    
    #[pointer_key(u16)] pub struct TxPointer(Height);
    #[pointer_key(u16)] pub struct UtxoPointer(TxPointer);
    #[pointer_key(u16)] pub struct InputPointer(TxPointer);
    #[pointer_key(u8)] pub struct AssetPointer(UtxoPointer);
    
    #[index] pub struct Hash(pub String);
    #[index] pub struct Address(pub String);
    #[index] pub struct Datum(pub String);
    #[index] pub struct PolicyId(pub String);
    #[index] pub struct AssetName(pub String);
    
    #[entity]
    pub struct Block {
        #[pk(range)]
        pub id: Height,
        #[one2one]
        pub header: BlockHeader,
        #[one2many]
        pub transactions: Vec<Transaction>,
    }
    
    #[entity]
    pub struct BlockHeader {
        #[fk(one2one, range)]
        pub id: Height,
        #[column(index)]
        pub hash: Hash,
        #[column(index, range)]
        pub timestamp: u32,
        #[column(index)]
        pub merkle_root: Hash,
        #[column]
        pub nonce: u64,
    }
    
    #[entity]
    pub struct Transaction {
        #[fk(one2many, range)]
        pub id: TxPointer,
        #[column(index)]
        pub hash: Hash,
        #[one2many]
        pub utxos: Vec<Utxo>,
        #[one2many]
        pub inputs: Vec<InputRef>,
    }
    
    #[entity]
    pub struct Utxo {
        #[fk(one2many, range)]
        pub id: UtxoPointer,
        #[column]
        pub amount: u64,
        #[column(index)]
        pub datum: Datum,
        #[column(index, dictionary)]
        pub address: Address,
        #[one2many]
        pub assets: Vec<Asset>,
    }
    
    #[entity]
    pub struct InputRef {
        #[fk(one2many, range)]
        pub id: InputPointer,
    }
    
    #[entity]
    pub struct Asset {
        #[fk(one2many, range)]
        pub id: AssetPointer,
        #[column]
        pub amount: u64,
        #[column(index, dictionary)]
        pub name: AssetName,
        #[column(index, dictionary)]
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
    
    pub fn run(db: Arc<Database>) -> Result<(), AppError> {
        let blocks = Block::sample_many(2);
    
        println!("Persisting blocks:");
        let write_tx = db.begin_write()?;
        Block::store_many(&write_tx, &blocks)?;
        write_tx.commit()?;
        
        let read_tx = db.begin_read()?;
    
        println!("Querying blocks:");
        let first_block = Block::first(&read_tx)?.unwrap();
        let last_block = Block::last(&read_tx)?.unwrap();
    
        Block::take(&read_tx, 1000)?;
        Block::get(&read_tx, &first_block.id)?;
        Block::range(&read_tx, &first_block.id, &last_block.id)?;
        Block::get_transactions(&read_tx, &first_block.id)?;
        Block::get_header(&read_tx, &first_block.id)?;
        Block::exists(&read_tx, &first_block.id)?;
        Block::first(&read_tx)?;
        Block::last(&read_tx)?;
    
        println!("Querying block headers:");
        let first_block_header = BlockHeader::first(&read_tx)?.unwrap();
        let last_block_header = BlockHeader::last(&read_tx)?.unwrap();
    
        BlockHeader::take(&read_tx, 1000)?;
        BlockHeader::get(&read_tx, &first_block_header.id)?;
        BlockHeader::range(&read_tx, &first_block_header.id, &last_block_header.id)?;
        BlockHeader::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
        BlockHeader::get_by_hash(&read_tx, &first_block_header.hash)?;
        BlockHeader::get_by_timestamp(&read_tx, &first_block_header.timestamp)?;
        BlockHeader::get_by_merkle_root(&read_tx, &first_block_header.merkle_root)?;
    
        println!("Querying transactions:");
        let first_transaction = Transaction::first(&read_tx)?.unwrap();
        let last_transaction = Transaction::last(&read_tx)?.unwrap();
    
        Transaction::take(&read_tx, 1000)?;
        Transaction::get(&read_tx, &first_transaction.id)?;
        Transaction::get_by_hash(&read_tx, &first_transaction.hash)?;
        Transaction::range(&read_tx, &first_transaction.id, &last_transaction.id)?;
        Transaction::get_utxos(&read_tx, &first_transaction.id)?;
        Transaction::get_inputs(&read_tx, &first_transaction.id)?;
        Transaction::parent_pk(&read_tx, &first_transaction.id)?;
    
        println!("Querying utxos:");
        let first_utxo = Utxo::first(&read_tx)?.unwrap();
        let last_utxo = Utxo::last(&read_tx)?.unwrap();
    
        Utxo::take(&read_tx, 1000)?;
        Utxo::get(&read_tx, &first_utxo.id)?;
        Utxo::get_by_address(&read_tx, &first_utxo.address)?;
        Utxo::get_by_datum(&read_tx, &first_utxo.datum)?;
        Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id)?;
        Utxo::get_assets(&read_tx, &first_utxo.id)?;
        Utxo::parent_pk(&read_tx, &first_utxo.id)?;
    
        println!("Querying input refs:");
        let first_input_ref = InputRef::first(&read_tx)?.unwrap();
        let last_input_ref = InputRef::last(&read_tx)?.unwrap();
    
        InputRef::take(&read_tx, 1000)?;
        InputRef::exists(&read_tx, &first_input_ref.id)?;
        InputRef::get(&read_tx, &first_input_ref.id)?;
        InputRef::range(&read_tx, &first_input_ref.id, &last_input_ref.id)?;
        InputRef::parent_pk(&read_tx, &first_input_ref.id)?;
    
    
        println!("Querying assets:");
        let first_asset = Asset::first(&read_tx)?.unwrap();
        let last_asset = Asset::last(&read_tx)?.unwrap();
    
        Asset::take(&read_tx, 1000)?;
        Asset::get(&read_tx, &first_asset.id)?;
        Asset::get_by_name(&read_tx, &first_asset.name)?;
        Asset::get_by_policy_id(&read_tx, &first_asset.policy_id)?;
        Asset::range(&read_tx, &first_asset.id, &last_asset.id)?;
        Asset::parent_pk(&read_tx, &first_asset.id)?;
    
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
An operation on top of a 3 blocks of 10 transactions of 20 utxos of 3 assets, ie.
`Block__store_and_commit` and `Block__all` operations write/read :
- 3 blocks
- 3 * 10 = 30 transactions
- 3 * 10 * 20 = 600 utxos
- 3 * 10 * 20 * 3 = 1800 assets

Which means indexing `Bitcoin` is way faster than Bitcoin Core syncs itself.

<!-- BEGIN_BENCH -->
```
function                                           ops/s
-------------------------------------------------------------
Block__all                                          1970
Transaction__all                                    2035
Transaction__get_by_hash                            2052
Utxo__all                                           2091
Utxo__get_by_datum                                  2095
Utxo__get_by_address                                2107
Utxo__range                                         2186
Transaction__range                                  2301
Asset__all                                          2820
Asset__range                                        2858
Asset__get_by_name                                  2869
Asset__get_by_policy_id                             2876
Block__range                                        2972
Block__get                                          5930
Block__get_transactions                             6002
Transaction__get                                   18514
Transaction__get_utxos                             19471
Utxo__get                                          57419
Utxo__get_assets                                   74826
BlockHeader__get_by_merkle_root                   100101
BlockHeader__all                                  101386
BlockHeader__get_by_hash                          102755
BlockHeader__get_by_timestamp                     103433
BlockHeader__range                                142292
Asset__get                                        210834
Block__get_header                                 280401
BlockHeader__get                                  281610
BlockHeader__range_by_timestamp                  1358690
```
<!-- END_BENCH -->
