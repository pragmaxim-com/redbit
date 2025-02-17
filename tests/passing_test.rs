#![allow(warnings)]

use redbit::*;

#[derive(Redbit)]
struct MinimalStruct {
    #[pk]
    id: u32,
    #[column]
    persisted_no_index_no_dict: i32,
}

#[derive(Redbit)]
struct StructWithPersistedRedbitField {
    #[pk]
    id: u32,
    #[column(index)]
    persisted_indexed_no_dict: i32,
}

#[derive(Redbit)]
struct StructWithPersistedRedbitFieldWithDict {
    #[pk]
    id: u32,
    #[column(index, dictionary)]
    persisted_indexed_with_dict: i32,
}

#[derive(Redbit)]
pub struct FullStruct {
    #[pk]
    pub id: u32,
    #[column]
    pub amount: u32,
    #[column(index)]
    pub datum: String,
    #[column(index, dictionary)]
    pub address: String,
}

fn main() {
    let _ = MinimalStruct { id: 1, persisted_no_index_no_dict: 42 };

    let _ = StructWithPersistedRedbitField { id: 2, persisted_indexed_no_dict: 43 };
    let _ = StructWithPersistedRedbitFieldWithDict { id: 3, persisted_indexed_with_dict: 44 };
    let _ = FullStruct { id: 4, amount: 45, datum: "datum".to_string(), address: "address".to_string() };
}
