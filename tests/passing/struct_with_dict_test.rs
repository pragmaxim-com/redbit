#![allow(warnings)]
#![feature(test)]
extern crate test;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use redbit::*;

#[column] pub struct Index(pub u32);
#[root_key] pub struct PK(pub u32);

#[entity]
struct StructWithDictField {
    #[pk]
    id: PK,
    #[column(dictionary)]
    persisted_indexed_with_dict: Index,
}

fn main() {
    let _ = StructWithDictField { id: PK(3), persisted_indexed_with_dict: Index(44) };
}
