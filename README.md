Built for blazing fast persistence of terra bytes of structured data on a single machine
while offering rich querying capabilities, eg. bitcoin/blockchain data.

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
- ✅ Http server with REST API for all db operations auto-generated

Let's say we want to persist Utxo into Redb using Redbit, declare annotated Struct `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    pub mod data;
    pub mod types;
    pub mod demo;
    pub use data::*;
    pub use redbit::*;
    pub use types::*;
    
    use derive_more::From;
    use serde::{Deserialize, Serialize};
    
    #[derive(Entity, Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Block {
        #[pk(range)]
        pub id: BlockPointer,
        #[one2one]
        pub header: BlockHeader,
        #[one2many]
        pub transactions: Vec<Transaction>,
    }
    
    #[derive(Entity, Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
    
    #[derive(Entity, Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
    
    #[derive(Entity, Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
    
    #[derive(Entity, Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct InputRef {
        #[fk(one2many, range)]
        pub id: InputPointer,
    }
    
    #[derive(Entity, Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
    
    #[derive(PK, Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, From)]
    pub struct BlockPointer {
        pub height: Height,
    }
    
    #[derive(PK, Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub struct TxPointer {
        #[parent]
        pub block_pointer: BlockPointer,
        pub tx_index: TxIndex,
    }
    
    #[derive(PK, Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub struct UtxoPointer {
        #[parent]
        pub tx_pointer: TxPointer,
        pub utxo_index: UtxoIndex,
    }
    
    #[derive(PK, Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub struct InputPointer {
        #[parent]
        pub tx_pointer: TxPointer,
        pub utxo_index: UtxoIndex,
    }
    
    #[derive(PK, Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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
Block__store_and_commit                               44
Utxo__all                                             70
Block__all                                            80
Transaction__all                                      82
Transaction__all                                      82
Utxo__all                                             85
Block__all                                            86
Transaction__range                                    86
Utxo__range                                           86
Utxo__range                                           90
Transaction__range                                    91
Asset__range                                         118
Asset__range                                         119
Block__range                                         123
Asset__all                                           127
Block__range                                         130
Asset__all                                           214
Block__get_transactions                              246
Block__get                                           247
Block__get                                           256
Block__get_transactions                              267
Asset__get_by_policy_id                              285
Asset__get_by_policy_id                              358
Utxo__get_by_address                                 411
Asset__get_by_name                                   559
Transaction__get_by_hash                             832
Utxo__get_by_address                                 847
Transaction__get_by_hash                             888
Asset__get_by_name                                  1176
Utxo__get_by_datum                                  1247
Utxo__get_by_datum                                  1686
Transaction__get                                    2516
Transaction__get_utxos                              2539
Transaction__get                                    2695
Transaction__get_utxos                              2702
Utxo__get                                          50612
Utxo__get                                          53358
Utxo__get_assets                                   66102
Utxo__get_assets                                   70686
BlockHeader__get_by_merkle_root                    91748
BlockHeader__all                                   97561
BlockHeader__all                                  100746
BlockHeader__get_by_hash                          101181
BlockHeader__get_by_merkle_root                   101723
BlockHeader__get_by_hash                          106044
BlockHeader__range_by_timestamp                   124057
BlockHeader__range_by_timestamp                   136071
BlockHeader__range                                144002
BlockHeader__range                                147597
Asset__get                                        189053
Asset__get                                        197572
BlockHeader__get_by_timestamp                     219954
BlockHeader__get                                  237630
BlockHeader__get_by_timestamp                     265613
BlockHeader__get                                  272544
Block__get_header                                 277633
Block__get_header                                 299232
```
<!-- END_BENCH -->

### Http Endpoints generated
```
endpoint                                          description
-------------------------------------------------------------
GET:/block/id/{value}                             block_get
GET:/block?take=                                  block_take
GET:/block?first=                                 block_first
GET:/block?last=                                  block_last
HEAD:/block/id/{value}                            block_exists
GET:/block/id?from=&until=                        block_range
GET:/block/{value}/header                         block_get_header
GET:/block/{value}/transactions                   block_get_transactions

GET:/blockheader/id/{value}                       blockheader_get
GET:/blockheader?take=                            blockheader_take
GET:/blockheader?first=                           blockheader_first
GET:/blockheader?last=                            blockheader_last
HEAD:/blockheader/id/{value}                      blockheader_exists
GET:/blockheader/id?from=&until=                  blockheader_range
GET:/blockheader/hash/{value}                     blockheader_get_by_hash
GET:/blockheader/timestamp/{value}                blockheader_get_by_timestamp
GET:/blockheader/timestamp?from=&until=           blockheader_range_by_timestamp
GET:/blockheader/merkle_root/{value}              blockheader_get_by_merkle_root

GET:/transaction/id/{value}                       transaction_get
GET:/transaction?take=                            transaction_take
GET:/transaction?first=                           transaction_first
GET:/transaction?last=                            transaction_last
HEAD:/transaction/id/{value}                      transaction_exists
GET:/transaction/id/{value}/parent_pk             transaction_parent_pk
GET:/transaction/id?from=&until=                  transaction_range
GET:/transaction/hash/{value}                     transaction_get_by_hash
GET:/transaction/{value}/utxos                    transaction_get_utxos
GET:/transaction/{value}/inputs                   transaction_get_inputs

GET:/utxo/id/{value}                              utxo_get
GET:/utxo?take=                                   utxo_take
GET:/utxo?first=                                  utxo_first
GET:/utxo?last=                                   utxo_last
HEAD:/utxo/id/{value}                             utxo_exists
GET:/utxo/id/{value}/parent_pk                    utxo_parent_pk
GET:/utxo/id?from=&until=                         utxo_range
GET:/utxo/datum/{value}                           utxo_get_by_datum
GET:/utxo/address/{value}                         utxo_get_by_address
GET:/utxo/{value}/assets                          utxo_get_assets

GET:/inputref/id/{value}                          inputref_get
GET:/inputref?take=                               inputref_take
GET:/inputref?first=                              inputref_first
GET:/inputref?last=                               inputref_last
HEAD:/inputref/id/{value}                         inputref_exists
GET:/inputref/id/{value}/parent_pk                inputref_parent_pk
GET:/inputref/id?from=&until=                     inputref_range

GET:/asset/id/{value}                             asset_get
GET:/asset?take=                                  asset_take
GET:/asset?first=                                 asset_first
GET:/asset?last=                                  asset_last
HEAD:/asset/id/{value}                            asset_exists
GET:/asset/id/{value}/parent_pk                   asset_parent_pk
GET:/asset/id?from=&until=                        asset_range
GET:/asset/name/{value}                           asset_get_by_name
GET:/asset/policy_id/{value}                      asset_get_by_policy_id
```
