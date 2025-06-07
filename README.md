Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from 
[Redb](https://github.com/cberner/redb) using secondary indexes and dictionaries, served by [axum](https://github.com/tokio-rs/axum)
through auto-generated REST API.

### Main motivations
- ✅ Achieving more advanced querying capabilities with embedded KV stores is non-trivial  
- ✅ Absence of any existing db & http higher-level layer for structured data in embedded KV stores
- ✅ Handwriting custom codecs on byte-level is tedious and painful

### Major Out-of-the-Box Features

- ✅ Querying and ranging by secondary index
- ✅ Optional dictionaries for low cardinality fields
- ✅ One-to-One and One-to-Many entities with cascade read/write/delete
- ✅ All goodies including intuitive data ordering without writing custom codecs
- ✅ Http server with REST API for all db operations auto-generated

Let's say we want to persist Utxo into Redb using Redbit, declare annotated Struct `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    pub mod data;
    pub mod types;
    pub mod db_demo;
    
    pub use data::*;
    pub use redbit::*;
    pub use types::*;
    
    use serde::{Deserialize, Serialize};
    
    #[derive(Entity, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Block {
        #[pk(range)]
        pub id: BlockPointer,
        #[one2one]
        pub header: BlockHeader,
        #[one2many]
        pub transactions: Vec<Transaction>,
    }
    
    #[derive(Entity, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct BlockHeader {
        #[pk(range)]
        pub id: BlockPointer,
        #[column(index)]
        pub hash: Hash,
        #[column(index, range)]
        pub timestamp: Timestamp,
        #[column(index)]
        pub merkle_root: Hash,
        #[column]
        pub nonce: Nonce,
    }
    
    #[derive(Entity, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Transaction {
        #[pk(range)]
        pub id: TxPointer,
        #[column(index)]
        pub hash: Hash,
        #[one2many]
        pub utxos: Vec<Utxo>,
        #[one2many]
        pub inputs: Vec<InputRef>,
    }
    
    #[derive(Entity, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Utxo {
        #[pk(range)]
        pub id: UtxoPointer,
        #[column]
        pub amount: Amount,
        #[column(index)]
        pub datum: Datum,
        #[column(index, dictionary)]
        pub address: Address,
        #[one2many]
        pub assets: Vec<Asset>,
    }
    
    #[derive(Entity, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Asset {
        #[pk(range)]
        pub id: AssetPointer,
        #[column]
        pub amount: Amount,
        #[column(index, dictionary)]
        pub name: AssetName,
        #[column(index, dictionary)]
        pub policy_id: PolicyId,
    }
    
    #[derive(PK, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub struct BlockPointer {
        pub height: Height,
    }
    
    #[derive(PK, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub struct TxPointer {
        #[parent]
        pub block_pointer: BlockPointer,
        pub tx_index: TxIndex,
    }
    
    #[derive(PK, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub struct UtxoPointer {
        #[parent]
        pub tx_pointer: TxPointer,
        pub utxo_index: UtxoIndex,
    }
    
    #[derive(PK, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub struct InputPointer {
        #[parent]
        pub tx_pointer: TxPointer,
        pub utxo_index: UtxoIndex,
    }
    
    #[derive(Entity, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct InputRef {
        #[pk(range)]
        pub id: InputPointer,
    }
    
    #[derive(PK, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub struct AssetPointer {
        #[parent]
        pub utxo_pointer: UtxoPointer,
        pub asset_index: AssetIndex,
    }
```
<!-- END_LIB -->

And R/W entire instances efficiently using indexes and dictionaries `examples/utxo/src/main.rs`:  

<!-- BEGIN_MAIN -->
```rust
    use std::sync::Arc;
    use redb::Database;
    use redbit::AppError;
    use crate::*;
    
    pub fn run(db: Arc<Database>) -> Result<(), AppError> {
        let blocks = get_blocks(Height(1), 10, 10, 3);
    
        println!("Persisting blocks:");
        for block in blocks.iter() {
            Block::store_and_commit(&db, block)?
        }
    
        let read_tx = db.begin_read()?;
    
        println!("Querying blocks:");
        let first_block = Block::first(&read_tx)?.unwrap();
        let last_block = Block::last(&read_tx)?.unwrap();
    
        Block::take(&read_tx, 1000)?;
        Block::get(&read_tx, &first_block.id)?;
        Block::range(&read_tx, &first_block.id, &last_block.id)?;
        Block::get_transactions(&read_tx, &first_block.id)?;
        Block::get_header(&read_tx, &first_block.id)?;
    
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
    
        println!("Querying utxos:");
        let first_utxo = Utxo::first(&read_tx)?.unwrap();
        let last_utxo = Utxo::last(&read_tx)?.unwrap();
    
        Utxo::take(&read_tx, 1000)?;
        Utxo::get(&read_tx, &first_utxo.id)?;
        Utxo::get_by_address(&read_tx, &first_utxo.address)?;
        Utxo::get_by_datum(&read_tx, &first_utxo.datum)?;
        Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id)?;
        Utxo::get_assets(&read_tx, &first_utxo.id)?;
    
        println!("Querying assets:");
        let first_asset = Asset::first(&read_tx)?.unwrap();
        let last_asset = Asset::last(&read_tx)?.unwrap();
    
        Asset::take(&read_tx, 1000)?;
        Asset::get(&read_tx, &first_asset.id)?;
        Asset::get_by_name(&read_tx, &first_asset.name)?;
        Asset::get_by_policy_id(&read_tx, &first_asset.policy_id)?;
        Asset::range(&read_tx, &first_asset.id, &last_asset.id)?;
    
        println!("Deleting blocks:");
        for block in blocks.iter() {
            Block::delete_and_commit(&db, &block.id)?
        }
        Ok(())
    }
```
<!-- END_MAIN -->

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
Block__store_and_commit                               42
Block__all                                            81
Transaction__all                                      81
Utxo__all                                             82
Utxo__range                                           82
Transaction__range                                    84
Asset__range                                         112
Block__range                                         121
Asset__all                                           195
Block__get                                           241
Block__get_transactions                              242
Asset__get_by_policy_id                              331
Transaction__get_by_hash                             809
Utxo__get_by_address                                 814
Asset__get_by_name                                  1073
Utxo__get_by_datum                                  1628
Transaction__get_utxos                              2442
Transaction__get                                    2495
Utxo__get                                          50149
Utxo__get_assets                                   64579
BlockHeader__all                                   90536
BlockHeader__get_by_hash                           94823
BlockHeader__get_by_merkle_root                    95478
BlockHeader__range_by_timestamp                   122901
BlockHeader__range                                136249
Asset__get                                        187310
BlockHeader__get_by_timestamp                     246737
Block__get_header                                 269794
BlockHeader__get                                  275461
```
<!-- END_BENCH -->
