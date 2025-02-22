Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from redb
using secondary indexes and dictionaries, let's say we want to persist Utxo into Redb using Redbit :

Declare annotated Struct `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
```rust
    mod codec;
    
    pub use redbit::*;
    use std::fmt::Debug;
    
    pub type Amount = u64;
    pub type Timestamp = u64;
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
        pub nonce: u32,
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
    
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BlockPointer {
        pub height: Height,
    }
    
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TxPointer {
        pub block_pointer: BlockPointer,
        pub tx_index: TxIndex,
    }
    
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct UtxoPointer {
        pub tx_pointer: TxPointer,
        pub utxo_index: UtxoIndex,
    }
    
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AssetPointer {
        pub utxo_pointer: UtxoPointer,
        pub asset_index: AssetIndex,
    }
    
    impl PK<TxPointer> for BlockPointer {
        fn fk_range(&self) -> (TxPointer, TxPointer) {
            (TxPointer { block_pointer: self.clone(), tx_index: TxIndex::MIN }, TxPointer { block_pointer: self.clone(), tx_index: TxIndex::MAX })
        }
    }
    
    impl PK<UtxoPointer> for TxPointer {
        fn fk_range(&self) -> (UtxoPointer, UtxoPointer) {
            (UtxoPointer { tx_pointer: self.clone(), utxo_index: UtxoIndex::MIN }, UtxoPointer { tx_pointer: self.clone(), utxo_index: UtxoIndex::MAX })
        }
    }
    
    impl PK<AssetPointer> for UtxoPointer {
        fn fk_range(&self) -> (AssetPointer, AssetPointer) {
            (
                AssetPointer { utxo_pointer: self.clone(), asset_index: AssetIndex::MIN },
                AssetPointer { utxo_pointer: self.clone(), asset_index: AssetIndex::MAX },
            )
        }
    }
    
    pub fn persist_blocks(db: &redb::Database, block_count: Height, tx_count: TxIndex, utxo_count: UtxoIndex, asset_count: AssetIndex) -> Result<Vec<Block>, DbEngineError> {
        let timestamp = 1678296000;
        let block_hash = String::from("block_hash");
        let merkle_root = String::from("merkle_root");
        let mut blocks = Vec::new();
        for height in 0..block_count {
            let block_id = BlockPointer { height };
            let transactions: Vec<Transaction> = (0..tx_count)
                .map(|tx_index| {
                    let tx_id = TxPointer { block_pointer: block_id.clone(), tx_index };
                    let utxos: Vec<Utxo> = (0..utxo_count)
                        .map(|utxo_index| {
                            let assets: Vec<Asset> = (0..asset_count)
                                .map(|asset_index| Asset {
                                    id: AssetPointer { utxo_pointer: UtxoPointer { tx_pointer: tx_id.clone(), utxo_index }, asset_index },
                                    amount: 999_999,
                                    name: "low-medium cardinality".to_string(),
                                    policy_id: "low cardinality".to_string(),
                                })
                                .collect();
                            Utxo {
                                id: UtxoPointer { tx_pointer: tx_id.clone(), utxo_index },
                                amount: 999_999,
                                datum: "high cardinality".to_string(),
                                address: "low-medium cardinality".to_string(),
                                assets,
                            }
                        })
                        .collect();
                    Transaction { id: tx_id, hash: format!("tx_hash_{}", tx_index), utxos }
                })
                .collect();
    
            let header = BlockHeader { id: block_id.clone(), hash: block_hash.clone(), timestamp: timestamp+1, merkle_root: merkle_root.clone(), nonce: 0 };
            let block = Block { id: block_id, header, transactions };
            blocks.push(block.clone());
            Block::store_and_commit(&db, &block)?
        }
        Ok(blocks)
    }
```
<!-- END_LIB -->

And R/W entire instances efficiently using indexes and dictionaries `examples/utxo/src/main.rs`:  

<!-- BEGIN_MAIN -->
```rust
    use utxo::*;
    
    fn main() {
        let db = redb::Database::create(std::env::temp_dir().join("my_db.redb")).unwrap();
        persist_blocks(&db, 10, 10, 10, 10).unwrap();
        let read_tx = db.begin_read().unwrap();
    
        let first_block = Block::first(&read_tx).unwrap().unwrap();
        let last_block = Block::last(&read_tx).unwrap().unwrap();
    
        Block::all(&read_tx).unwrap();
        Block::get(&read_tx, &first_block.id).unwrap();
        Block::range(&read_tx, &first_block.id, &last_block.id).unwrap();
        Block::get_transactions(&read_tx, &first_block.id).unwrap();
        Block::get_header(&read_tx, &first_block.id).unwrap();
    
        let first_block_header = BlockHeader::first(&read_tx).unwrap().unwrap();
        let last_block_header = BlockHeader::last(&read_tx).unwrap().unwrap();
    
        BlockHeader::all(&read_tx).unwrap();
        BlockHeader::get(&read_tx, &first_block_header.id).unwrap();
        BlockHeader::range(&read_tx, &first_block_header.id, &last_block_header.id).unwrap();
        BlockHeader::range_by_timestamp(&read_tx, &first_block_header.timestamp, &last_block_header.timestamp).unwrap();
        BlockHeader::get_by_hash(&read_tx, &first_block_header.hash).unwrap();
        BlockHeader::get_by_timestamp(&read_tx, &first_block_header.timestamp).unwrap();
        BlockHeader::get_by_merkle_root(&read_tx, &first_block_header.merkle_root).unwrap();
    
        let first_transaction = Transaction::first(&read_tx).unwrap().unwrap();
        let last_transaction = Transaction::last(&read_tx).unwrap().unwrap();
    
        Transaction::all(&read_tx).unwrap();
        Transaction::get(&read_tx, &first_transaction.id).unwrap();
        Transaction::get_by_hash(&read_tx, &first_transaction.hash).unwrap();
        Transaction::range(&read_tx, &first_transaction.id, &last_transaction.id).unwrap();
        Transaction::get_utxos(&read_tx, &first_transaction.id).unwrap();
    
        let first_utxo = Utxo::first(&read_tx).unwrap().unwrap();
        let last_utxo = Utxo::last(&read_tx).unwrap().unwrap();
    
        Utxo::all(&read_tx).unwrap();
        Utxo::get(&read_tx, &first_utxo.id).unwrap();
        Utxo::get_by_address(&read_tx, &first_utxo.address).unwrap();
        Utxo::get_by_datum(&read_tx, &first_utxo.datum).unwrap();
        Utxo::range(&read_tx, &first_utxo.id, &last_utxo.id).unwrap();
        Utxo::get_assets(&read_tx, &first_utxo.id).unwrap();
    
        let first_asset = Asset::first(&read_tx).unwrap().unwrap();
        let last_asset = Asset::last(&read_tx).unwrap().unwrap();
    
        Asset::all(&read_tx).unwrap();
        Asset::get(&read_tx, &first_asset.id).unwrap();
        Asset::get_by_name(&read_tx, &first_asset.name).unwrap();
        Asset::get_by_policy_id(&read_tx, &first_asset.policy_id).unwrap();
        Asset::range(&read_tx, &first_asset.id, &last_asset.id).unwrap();
    }
```
<!-- END_MAIN -->
