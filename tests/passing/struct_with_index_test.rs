#![allow(warnings)]
#![feature(test)]
extern crate test;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use redbit::*;

#[column] pub struct Index(pub u32);
#[root_key] pub struct PK(pub u32);

#[entity]
struct StructWithIndexField {
    #[pk]
    id: PK,
    #[column(index)]
    persisted_indexed_no_dict: Index,
}

fn main() {
    let _ = StructWithIndexField { id: PK(2), persisted_indexed_no_dict: Index(43) };
}
