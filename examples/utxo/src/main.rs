use utxo::*;

fn main() {
    let db = redb::Database::create(std::env::temp_dir().join("my_db.redb")).unwrap();

    let utxo = Utxo {
        id: UtxoPointer { block_height: 42,tx_index: 7, utxo_index: 6 },
        amount: 999_999,
        datum: String::from("high cardinality"),
        address: String::from("low-medium cardinality"),
    };

    let _ = Utxo::store(&db, &utxo).unwrap();
    let utxo = Utxo::get_by_id(&db, &utxo.id).unwrap();
    let utxos_by_address = Utxo::get_by_address(&db, &utxo.address).unwrap();
    let utxos_by_datum = Utxo::get_by_datum(&db, &utxo.datum).unwrap();

    println!("{:?}", utxo);
    println!("{:?}", utxos_by_address);
    println!("{:?}", utxos_by_datum);
}
