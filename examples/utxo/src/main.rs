use utxo::*;

fn main() {
    let db = redb::Database::create(std::env::temp_dir().join("my_db.redb")).unwrap();

    let height = 42;
    let timestamp = 1678296000;
    let block_id = BlockPointer { height };
    let block_hash = String::from("block_hash");

    let transactions: Vec<Transaction> = (0..1)
        .map(|tx_index| {
            let tx_id = TxPointer { block_pointer: block_id.clone(), tx_index };
            let utxos: Vec<Utxo> = (0..2)
                .map(|utxo_index| {
                    let assets: Vec<Asset> = (0..2)
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

    let merkle_root = String::from("merkle_root");
    let header = BlockHeader { id: block_id.clone(), hash: block_hash.clone(), timestamp, merkle_root: merkle_root.clone(), nonce: 0 };
    let block = Block { id: block_id.clone(), header, transactions };
    let _ = Block::store_and_commit(&db, &block).unwrap();

    let read_tx = db.begin_read().unwrap();

    Block::get(&read_tx, &block.id).unwrap();
    Block::range(&read_tx, &block.id, &block.id).unwrap();
    Block::get_transactions(&read_tx, &block.id).unwrap();
    Block::get_header(&read_tx, &block.id).unwrap();

    BlockHeader::get(&read_tx, &block.id).unwrap();
    BlockHeader::range(&read_tx, &block.id, &block.id).unwrap();
    BlockHeader::range_by_timestamp(&read_tx, &timestamp, &timestamp).unwrap();
    BlockHeader::get_by_hash(&read_tx, &block_hash).unwrap();
    BlockHeader::get_by_timestamp(&read_tx, &timestamp).unwrap();
    BlockHeader::get_by_merkle_root(&read_tx, &merkle_root).unwrap();

    Transaction::get(&read_tx, &block.transactions.first().unwrap().id).unwrap();
    Transaction::get_by_hash(&read_tx, &block.transactions.first().unwrap().hash).unwrap();
    Transaction::range(&read_tx, &block.transactions.first().unwrap().id, &block.transactions.last().unwrap().id).unwrap();
    Transaction::get_utxos(&read_tx, &block.transactions.first().unwrap().id).unwrap();

    let utxo = Utxo::get(&read_tx, &block.transactions.first().unwrap().utxos.first().unwrap().id).unwrap();
    Utxo::get_by_address(&read_tx, &utxo.address).unwrap();
    Utxo::get_by_datum(&read_tx, &utxo.datum).unwrap();
    Utxo::range(&read_tx, &utxo.id, &utxo.id).unwrap();

    let asset = Asset::get(&read_tx, &utxo.assets.first().unwrap().id).unwrap();
    Asset::get_by_name(&read_tx, &asset.name).unwrap();
    Asset::get_by_policy_id(&read_tx, &asset.policy_id).unwrap();
    Asset::range(&read_tx, &asset.id, &asset.id).unwrap();
}
