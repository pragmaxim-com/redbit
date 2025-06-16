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
- ✅ auto-generated http rest API at http://127.0.0.1:8000/swagger-ui/

Let's say we want to persist and query blockchain data using Redbit, declare annotated Structs `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    pub mod data;
    pub mod types;
    pub mod demo;
    pub use data::*;
    pub use redbit::*;
    pub use types::*;
    
    #[entity]
    pub struct Block {
        #[pk(range)]
        pub id: BlockPointer,
        #[one2one]
        pub header: BlockHeader,
        #[one2many]
        pub transactions: Vec<Transaction>,
    }
    
    #[entity]
    pub struct BlockHeader {
        #[fk(one2one, range)]
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
        pub amount: Amount,
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
        pub amount: Amount,
        #[column(index, dictionary)]
        pub name: AssetName,
        #[column(index, dictionary)]
        pub policy_id: PolicyId,
    }
    
    #[key]
    pub struct BlockPointer {
        pub height: Height,
    }
    
    #[key]
    pub struct TxPointer {
        #[parent]
        pub block_pointer: BlockPointer,
        pub tx_index: TxIndex,
    }
    
    #[key]
    pub struct UtxoPointer {
        #[parent]
        pub tx_pointer: TxPointer,
        pub utxo_index: UtxoIndex,
    }
    
    #[key]
    pub struct InputPointer {
        #[parent]
        pub tx_pointer: TxPointer,
        pub utxo_index: UtxoIndex,
    }
    
    #[key]
    pub struct AssetPointer {
        #[parent]
        pub utxo_pointer: UtxoPointer,
        pub asset_index: AssetIndex,
    }
```
<!-- END_LIB -->

And R/W entire instances efficiently using indexes and dictionaries `examples/utxo/src/demo.rs`:

<!-- BEGIN_MAIN -->
```rust
    use std::sync::Arc;
    use redb::Database;
    use redbit::AppError;
    use crate::*;
    
    pub fn run(db: Arc<Database>) -> Result<(), AppError> {
        let blocks = get_blocks(Height(1), 10, 10, 3);
    
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
Block__store_and_commit                               35
Block__store_and_commit                               43
Utxo__all                                             70
Block__all                                            82
Transaction__all                                      82
Transaction__all                                      82
Transaction__range                                    84
Utxo__all                                             84
Utxo__range                                           85
Block__all                                            86
Utxo__range                                           90
Transaction__range                                    91
Asset__range                                         118
Asset__range                                         119
Block__range                                         122
Asset__all                                           127
Block__range                                         130
Asset__all                                           213
Block__get                                           238
Block__get_transactions                              247
Block__get                                           256
Block__get_transactions                              267
Asset__get_by_policy_id                              285
Asset__get_by_policy_id                              352
Utxo__get_by_address                                 411
Asset__get_by_name                                   559
Transaction__get_by_hash                             823
Utxo__get_by_address                                 839
Transaction__get_by_hash                             888
Asset__get_by_name                                  1162
Utxo__get_by_datum                                  1247
Utxo__get_by_datum                                  1668
Transaction__get                                    2486
Transaction__get_utxos                              2541
Transaction__get                                    2695
Transaction__get_utxos                              2702
Utxo__get                                          51305
Utxo__get                                          53358
Utxo__get_assets                                   67708
Utxo__get_assets                                   70686
BlockHeader__get_by_merkle_root                    91748
BlockHeader__all                                   97561
BlockHeader__get_by_merkle_root                    99916
BlockHeader__get_by_hash                          101425
BlockHeader__all                                  101679
BlockHeader__get_by_hash                          106044
BlockHeader__range_by_timestamp                   124057
BlockHeader__range_by_timestamp                   134169
BlockHeader__range                                144804
BlockHeader__range                                147597
Asset__get                                        191860
Asset__get                                        197572
BlockHeader__get_by_timestamp                     219954
BlockHeader__get                                  237630
BlockHeader__get_by_timestamp                     264348
Block__get_header                                 277703
BlockHeader__get                                  287069
Block__get_header                                 299232
```
<!-- END_BENCH -->
