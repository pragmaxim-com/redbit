#![allow(warnings)]
#![feature(test)]
extern crate test;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use redbit::*;

#[column] pub struct Address(pub String);
#[column] pub struct Datum(pub String);
#[column] pub struct Index(pub u32);
#[root_key] pub struct PK(pub u32);

#[entity]
pub struct FullStruct {
    #[pk]
    pub id: PK,
    #[column]
    pub amount: u32,
    #[column(index)]
    pub datum: Datum,
    #[column(dictionary)]
    pub address: Address,
}

fn main() {
    let _ = FullStruct { id: PK(4), amount: 45, datum: Datum::default(), address: Address::default() };
}
