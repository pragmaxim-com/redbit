Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from redb
using secondary indexes and dictionaries, let's say we want to persist Utxo into Redb using Redbit :

### Main motivations
- ✅ Achieving more advanced querying capabilities with embedded KV stores is non-trivial  
- ✅ Absence of any existing abstraction layer for structured data  
- ✅ Handwriting custom codecs on byte-level is tedious and painful

### Major Out-of-the-Box Features

- ✅ Querying and ranging by secondary index
- ✅ Optional dictionaries for low cardinality fields
- ✅ One-to-One and One-to-Many entities with cascade read/write/delete
- ✅ All goodies including intuitive data ordering without writing custom codecs

Declare annotated Struct `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    mod data;
    
    pub use data::*;
    pub use redbit::*;
    
    use serde::{Deserialize, Serialize};
    use std::fmt::Debug;
    
    pub type Amount = u64;
    pub type Timestamp = u64;
    pub type Nonce = u32;
    pub type Height = u32;
    pub type TxIndex = u16;
    pub type UtxoIndex = u16;
    pub type AssetIndex = u16;
    pub type Datum = String;
    pub type Address = String;
    pub type AssetName = String;
    pub type PolicyId = String;
    pub type Hash = String;
    
    #[derive(Entity, Debug, Clone, PartialEq, Eq)]
    pub struct Block {
        #[pk(range)]
        pub id: BlockPointer,
        #[one2one]
        pub header: BlockHeader,
        #[one2many]
        pub transactions: Vec<Transaction>,
    }
    
    #[derive(Entity, Debug, Clone, PartialEq, Eq)]
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
    
    #[derive(Entity, Debug, Clone, PartialEq, Eq)]
    pub struct Transaction {
        #[pk(range)]
        pub id: TxPointer,
        #[column(index)]
        pub hash: Hash,
        #[one2many]
        pub utxos: Vec<Utxo>,
    }
    
    #[derive(Entity, Debug, Clone, PartialEq, Eq)]
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
    
    #[derive(Entity, Debug, Clone, PartialEq, Eq)]
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
    use utxo::*;
    
    fn demo() -> Result<(), DbEngineError> {
        let db = redb::Database::create(std::env::temp_dir().join("my_db.redb"))?;
        let blocks = get_blocks(1, 10, 10, 3);
    
        println!("Persisting blocks:");
        for block in blocks.iter() {
            Block::store_and_commit(&db, block)?
        }
    
        let read_tx = db.begin_read()?;
    
        println!("Querying blocks:");
        let first_block = Block::first(&read_tx)?.unwrap();
        let last_block = Block::last(&read_tx)?.unwrap();
    
        Block::all(&read_tx)?;
        Block::get(&read_tx, &first_block.id)?;
        Block::range(&read_tx, &first_block.id, &last_block.id)?;
        Block::get_transactions(&read_tx, &first_block.id)?;
        Block::get_header(&read_tx, &first_block.id)?;
    
        println!("Querying block headers:");
        let first_block_header = BlockHeader::first(&read_tx)?.unwrap();
        let last_block_header = BlockHeader::last(&read_tx)?.unwrap();
    
        BlockHeader::all(&read_tx)?;
        BlockHeader::get(&read_tx, &first_block_header.id)?;
        BlockHeader::range(&read_tx, &first_block_header.id, &last_block_header.id)?;
        BlockHeader::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp)?;
        BlockHeader::get_by_hash(&read_tx, &first_block_header.hash)?;
        BlockHeader::get_by_timestamp(&read_tx, &first_block_header.timestamp)?;
        BlockHeader::get_by_merkle_root(&read_tx, &first_block_header.merkle_root)?;
    
        println!("Querying transactions:");
        let first_transaction = Transaction::first(&read_tx)?.unwrap();
        let last_transaction = Transaction::last(&read_tx)?.unwrap();
    
        Transaction::all(&read_tx)?;
        Transaction::get(&read_tx, &first_transaction.id)?;
        Transaction::get_by_hash(&read_tx, &first_transaction.hash)?;
        Transaction::range(&read_tx, &first_transaction.id, &last_transaction.id)?;
        Transaction::get_utxos(&read_tx, &first_transaction.id)?;
    
        println!("Querying utxos:");
        let first_utxo = Utxo::first(&read_tx)?.unwrap();
        let last_utxo = Utxo::last(&read_tx)?.unwrap();
    
        Utxo::all(&read_tx)?;
        Utxo::get(&read_tx, &first_utxo.id)?;
        Utxo::get_by_address(&read_tx, &first_utxo.address)?;
        Utxo::get_by_datum(&read_tx, &first_utxo.datum)?;
        Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id)?;
        Utxo::get_assets(&read_tx, &first_utxo.id)?;
    
        println!("Querying assets:");
        let first_asset = Asset::first(&read_tx)?.unwrap();
        let last_asset = Asset::last(&read_tx)?.unwrap();
    
        Asset::all(&read_tx)?;
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
    
    fn main() {
        demo().unwrap();
    }
```
<!-- END_MAIN -->

Performance wise, check 🔥[flamegraph](https://rawcdn.githack.com/pragmaxim-com/redbit/refs/heads/master/flamegraph.svg).
Instances are persisted completely structured by fields which means Redbit has slower write performance but blazing fast reads.


### ⏱️ Benchmark Summary
An operation on top of a 3 blocks of 10 transactions of 20 utxos of 3 assets
```csv
function,ops/s
Block__store_and_commit,43
Block__all,82
Transaction__all,82
Transaction__range,84
Utxo__all,84
Utxo__range,84
Asset__all,121
Asset__range,121
Block__range,122
Block__get,241
Block__get_transactions,245
Asset__get_by_policy_id,362
Transaction__get_by_hash,826
Utxo__get_by_address,835
Asset__get_by_name,1203
Utxo__get_by_datum,1651
Transaction__get_utxos,2308
Transaction__get,2469
Utxo__get,49033
Utxo__get_assets,66028
BlockHeader__get_by_merkle_root,97212
BlockHeader__all,97812
BlockHeader__get_by_hash,98412
BlockHeader__range_by_timestamp,135309
BlockHeader__range,140026
Asset__get,185808
BlockHeader__get_by_timestamp,260209
Block__get_header,277892
BlockHeader__get,278489
```
