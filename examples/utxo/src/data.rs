use std::env;
use rand::random;
use redb::Database;
use crate::*;

pub fn empty_temp_db(name: &str) -> Database {
    let dir = env::temp_dir().join("redbit");
    if !dir.exists() {
        std::fs::create_dir_all(dir.clone()).unwrap();
    }
    let db_path = dir.join(format!("{}_{}.redb", name, random::<u64>()));
    Database::create(db_path).expect("Failed to create database")
}

pub fn init_temp_db(name: &str) -> (Vec<Block>, Database) {
    let db = empty_temp_db(name);
    let blocks = get_blocks(Height(4), 4, 4, 4);
    blocks.iter().for_each(|block| Block::store_and_commit(&db, &block).expect("Failed to persist blocks"));
    (blocks, db)
}

pub fn get_blocks(block_count: Height, tx_count: TxIndex, utxo_count: UtxoIndex, asset_count: AssetIndex) -> Vec<Block> {
    let timestamp = 1678296000;
    let block_hash = String::from("block_hash");
    let merkle_root = String::from("merkle_root");
    (0..block_count.0)
        .map(|height| {
            let block_id = BlockPointer { height: Height(height) };
            let transactions: Vec<Transaction> = (0..tx_count)
                .map(|tx_index| {
                    let tx_id = TxPointer { block_pointer: block_id.clone(), tx_index };
                    let utxos: Vec<Utxo> = (0..utxo_count)
                        .map(|utxo_index| {
                            let assets: Vec<Asset> = (0..asset_count)
                                .map(|asset_index| Asset {
                                    id: AssetPointer { utxo_pointer: UtxoPointer { tx_pointer: tx_id.clone(), utxo_index }, asset_index },
                                    amount: 999_999,
                                    name: format!("medium cardinality_{}", tx_index),
                                    policy_id: format!("low cardinality_{}", height),
                                })
                                .collect();
                            Utxo {
                                id: UtxoPointer { tx_pointer: tx_id.clone(), utxo_index },
                                amount: 999_999,
                                datum: format!("high cardinality_{}", utxo_index),
                                address: format!("medium cardinality_{}", tx_index),
                                assets,
                            }
                        })
                        .collect();
                    let input: InputRef = InputRef {
                        id: InputPointer { tx_pointer: tx_id.clone(), utxo_index: 0 },
                    };
                    Transaction { id: tx_id, hash: format!("tx_hash_{}", tx_index), utxos, inputs: vec![input] }
                })
                .collect();

            Block {
                id: block_id.clone(),
                header: BlockHeader { id: block_id, hash: block_hash.clone(), timestamp: Timestamp(timestamp + u32::from(height)), merkle_root: merkle_root.clone(), nonce: 0 },
                transactions,
            }
        })
        .collect()
}

// make a singleton instance that will hold get_blocks data
