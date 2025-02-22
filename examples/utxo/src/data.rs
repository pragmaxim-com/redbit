use redbit::DbEngineError;
use crate::*;

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