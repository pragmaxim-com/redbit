#![allow(warnings)]
#![feature(test)]
extern crate test;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use redbit::*;

#[column] pub struct Address(pub String);
#[column] pub struct Datum(pub String);
#[column] pub struct Index(pub u32);

#[root_key] struct ParentPK(u32);
#[pointer_key] struct MultipleOne2ManyPointer(ParentPK);

#[entity]
struct MultipleOne2Many {
    #[pk]
    id: ParentPK,
    ones: Vec<One>,
    twos: Vec<Two>,
}

#[entity]
struct One {
    #[fk(one2many)]
    pub id: MultipleOne2ManyPointer,
    #[column]
    pub persisted_no_index_no_dict: u32,
}

#[entity]
struct Two {
    #[fk(one2many)]
    pub id: MultipleOne2ManyPointer,
    #[column]
    pub persisted_no_index_no_dict: u32,
}

fn main() {
    let parent_pointer = ParentPK(5);
    let pointer_0 = MultipleOne2ManyPointer::from_parent(parent_pointer, 0);
    let pointer_1 = pointer_0.next_index();
    let _ = Two { id: pointer_0, persisted_no_index_no_dict: 42 };
    let _ = MultipleOne2Many { id: parent_pointer, ones: vec![One { id: pointer_0, persisted_no_index_no_dict: 46 }], twos: vec![Two { id: pointer_1, persisted_no_index_no_dict: 47 }] };
}
