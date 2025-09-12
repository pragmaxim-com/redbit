#![allow(warnings)]
#![feature(test)]
extern crate test;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use redbit::*;

#[root_key] pub struct PK(pub u32);

#[entity]
struct TransientAnnotationStruct {
    #[pk]
    id: PK,
    #[column]
    pub foo: u32,
    #[column(transient)]
    name: String
}
fn main() {
    let _ = TransientAnnotationStruct { id: PK(1), name: "foo".to_string(), foo: 3};
}
