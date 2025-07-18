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
#[pointer_key] struct MultipleOne2ManyAnnotationsStructPointer(ParentPK);

#[entity]
struct MultipleOne2ManyAnnotationsStruct {
    #[pk]
    id: ParentPK,
    foos: Vec<MinimalStruct>,
    bars: Vec<MinimalStruct>,
}

#[entity]
struct MinimalStruct {
    #[fk(one2many)]
    pub id: MultipleOne2ManyAnnotationsStructPointer,
    #[column]
    pub persisted_no_index_no_dict: u32,
}

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


#[entity]
struct StructWithPersistedEntityField {
    #[pk]
    id: PK,
    #[column(index)]
    persisted_indexed_no_dict: Index,
}

#[entity]
struct StructWithPersistedEntityFieldWithDict {
    #[pk]
    id: PK,
    #[column(dictionary)]
    persisted_indexed_with_dict: Index,
}

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
    let parent_pointer = ParentPK(5);
    let pointer_0 = MultipleOne2ManyAnnotationsStructPointer::from_parent(parent_pointer.clone(), 0);
    let pointer_1 = pointer_0.next_index();
    let _ = MinimalStruct { id: pointer_0.clone(), persisted_no_index_no_dict: 42 };
    let _ = StructWithPersistedEntityField { id: PK(2), persisted_indexed_no_dict: Index(43) };
    let _ = StructWithPersistedEntityFieldWithDict { id: PK(3), persisted_indexed_with_dict: Index(44) };
    let _ = FullStruct { id: PK(4), amount: 45, datum: Datum::default(), address: Address::default() };
    let _ = MultipleOne2ManyAnnotationsStruct { id: parent_pointer, foos: vec![MinimalStruct { id: pointer_0.clone(), persisted_no_index_no_dict: 46 }], bars: vec![MinimalStruct { id: pointer_1, persisted_no_index_no_dict: 47 }] };
    let _ = TransientAnnotationStruct { id: PK(1), name: "foo".to_string(), foo: 3};
}
