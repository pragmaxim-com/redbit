use utxo::*;

fn create_test_db() -> redb::Database {
    let random_number = rand::random::<u32>();
    redb::Database::create(std::env::temp_dir().join(format!("test_db_{}.redb", random_number))).unwrap()
}

fn create_test_utxo(block_height: u32, tx_index: u16, utxo_index: u16) -> Utxo {
    Utxo {
        id: UtxoPointer {
            block_height,
            tx_index,
            utxo_index,
        }.into(),
        amount: 999_999,
        datum: format!("datum_{}", block_height),
        address: format!("address_{}", tx_index),
    }
}

#[test]
fn it_should_get_entity_by_unique_id() {
    let db = create_test_db();
    let utxo = create_test_utxo(42, 7, 6);

    Utxo::store(&db, &utxo).expect("Failed to store utxo");

    let found_by_id = Utxo::get_by_id(&db, &utxo.id).expect("Failed to query by ID");
    assert_eq!(found_by_id.id, utxo.id);
    assert_eq!(found_by_id.amount, utxo.amount);
    assert_eq!(found_by_id.datum, utxo.datum);
    assert_eq!(found_by_id.address, utxo.address);
}

#[test]
fn it_should_get_entities_by_index_with_one_to_many_relationship() {
    let db = create_test_db();
    let utxo1 = create_test_utxo(42, 7, 6);
    let utxo2 = create_test_utxo(43, 7, 1);

    Utxo::store(&db, &utxo1).expect("Failed to store utxo1");
    Utxo::store(&db, &utxo2).expect("Failed to store utxo2");

    // Test by address (one-to-many)
    let found_by_address = Utxo::get_by_address(&db, &utxo1.address).expect("Failed to query by address");
    assert_eq!(found_by_address.len(), 2);
    assert!(found_by_address.iter().any(|u| u.id == utxo1.id));
    assert!(found_by_address.iter().any(|u| u.id == utxo2.id));

    // Test by datum (one-to-many)
    let found_by_datum1 = Utxo::get_by_datum(&db, &utxo1.datum).expect("Failed to query by datum");
    assert_eq!(found_by_datum1.len(), 1);
    assert_eq!(found_by_datum1[0].id, utxo1.id);

    let found_by_datum2 = Utxo::get_by_datum(&db, &utxo2.datum).expect("Failed to query by datum");
    assert_eq!(found_by_datum2.len(), 1);
    assert_eq!(found_by_datum2[0].id, utxo2.id);
}

#[test]
fn it_should_override_entity_under_existing_unique_id() {
    let db = create_test_db();
    let mut utxo = create_test_utxo(42, 7, 6);

    Utxo::store(&db, &utxo).expect("Failed to store initial utxo");

    // Modify the UTXO
    utxo.amount = 1_000_000;
    utxo.datum = String::from("updated_datum");
    utxo.address = String::from("updated_address");

    // Store the modified UTXO (should override the existing one)
    Utxo::store(&db, &utxo).expect("Failed to store updated utxo");

    // Retrieve and verify the updated UTXO
    let updated_utxo = Utxo::get_by_id(&db, &utxo.id).expect("Failed to query updated UTXO");
    assert_eq!(updated_utxo.id, utxo.id);
    assert_eq!(updated_utxo.amount, 1_000_000);
    assert_eq!(updated_utxo.datum, "updated_datum");
    assert_eq!(updated_utxo.address, "updated_address");
}

#[test]
fn compile_pass_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/passing_test.rs");
}

#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/failing/*.rs");
}
