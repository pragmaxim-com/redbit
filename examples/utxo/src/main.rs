use utxo::*;

fn main() {
    let db = redb::Database::create(std::env::temp_dir().join("my_db.redb")).unwrap();

    let block_height = 42;
    let block = Block { hash: String::from("unique"), height: block_height, timestamp: 1678296000 };

    let utxo = Utxo {
        id: UtxoPointer { block_height, tx_index: 7, utxo_index: 6 },
        amount: 999_999,
        datum: String::from("high cardinality"),
        address: String::from("low-medium cardinality"),
    };

    let _ = Block::store(&db, &block).unwrap();
    let block = Block::get_by_hash(&db, &block.hash).unwrap();
    let blocks_by_height = Block::get_by_height(&db, &block_height).unwrap();
    let block_range_by_height = Block::range_by_height(&db, &block_height, &(block_height + 1)).unwrap();

    println!("{:?}", block);
    println!("{:?}", blocks_by_height);
    println!("{:?}", block_range_by_height);

    let _ = Utxo::store(&db, &utxo).unwrap();
    let utxo = Utxo::get_by_id(&db, &utxo.id).unwrap();
    let utxos_by_address = Utxo::get_by_address(&db, &utxo.address).unwrap();
    let utxos_by_datum = Utxo::get_by_datum(&db, &utxo.datum).unwrap();
    let utxo_range_by_ids = Utxo::range_by_id(&db, &utxo.id, &utxo.id).unwrap();

    println!("{:?}", utxo);
    println!("{:?}", utxos_by_address);
    println!("{:?}", utxos_by_datum);
    println!("{:?}", utxo_range_by_ids);
}
