use utxo::*;

fn main() {
    let db = redb::Database::create(std::env::temp_dir().join("my_db.redb")).unwrap();

    let height = 42;
    let timestamp= 1678296000;
    let block_id = BlockPointer{height};
    let block_hash = String::from("block_hash");

    let transactions: Vec<Transaction> = (0..1)
        .map(|tx_index| {
            let tx_id = TxPointer { block_pointer: block_id.clone(), tx_index };
            let utxos: Vec<Utxo> = (0..2)
                .map(|utxo_index| Utxo {
                    id: UtxoPointer { tx_pointer: tx_id.clone(), utxo_index },
                    amount: 999_999,
                    datum: "high cardinality".to_string(),
                    address: "low-medium cardinality".to_string(),
                })
                .collect();
            Transaction {
                id: tx_id,
                hash: format!("tx_hash_{}", tx_index),
                utxos,
            }
        })
        .collect();

    let block = Block {
        id: block_id.clone(),
        hash: block_hash.clone(),
        timestamp,
        transactions,
    };
    let _ = Block::store_and_commit(&db, &block).unwrap();

    let read_tx = db.begin_read().unwrap();

    let _block = Block::get_by_id(&read_tx, &block.id).unwrap();
    let _block_range_by_ids = Block::range(&read_tx, &block.id, &block.id).unwrap();
    let _block_range_by_ids = Block::range_by_timestamp(&read_tx, &timestamp, &timestamp).unwrap();
    let _blocks = Block::get_by_hash(&read_tx, &block_hash).unwrap();
    let _blocks_by_height = Block::get_by_timestamp(&read_tx, &timestamp).unwrap();
    let _block_transactions = Block::get_transactions(&read_tx, &block.id).unwrap();

    let _transactions = Transaction::get_by_id(&read_tx, &block.transactions.first().unwrap().id).unwrap();
    let _transactions_by_timestamp = Transaction::get_by_hash(&read_tx, &block.transactions.first().unwrap().hash).unwrap();
    let _transaction_range_by_ids = Transaction::range(&read_tx, &block.transactions.first().unwrap().id, &block.transactions.last().unwrap().id).unwrap();
    let _transaction_utxos = Transaction::get_utxos(&read_tx, &block.transactions.first().unwrap().id).unwrap();

    let utxo = Utxo::get_by_id(&read_tx, &block.transactions.first().unwrap().utxos.first().unwrap().id).unwrap();
    let _utxos_by_address = Utxo::get_by_address(&read_tx, &utxo.address).unwrap();
    let _utxos_by_datum = Utxo::get_by_datum(&read_tx, &utxo.datum).unwrap();
    let _utxo_range_by_ids = Utxo::range(&read_tx, &utxo.id, &utxo.id).unwrap();
}

