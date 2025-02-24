use crate::*;

pub fn get_blocks(block_count: Height, tx_count: TxIndex, utxo_count: UtxoIndex, asset_count: AssetIndex) -> Vec<Block> {
    let timestamp = 1678296000;
    let block_hash = String::from("block_hash");
    let merkle_root = String::from("merkle_root");
    (0..block_count)
        .map(|height| {
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

            Block {
                id: block_id.clone(),
                header: BlockHeader { id: block_id, hash: block_hash.clone(), timestamp: timestamp + u64::from(height), merkle_root: merkle_root.clone(), nonce: 0 },
                transactions,
            }
        })
        .collect()
}
