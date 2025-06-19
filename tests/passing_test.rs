#![allow(warnings)]

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use redbit::*;

#[indexed_column] pub struct Address(pub String);
#[indexed_column] pub struct Datum(pub String);

#[entity]
struct MinimalStruct {
    #[fk(one2many, range)]
    pub id: ChildPK,
    #[column]
    pub persisted_no_index_no_dict: i32,
}

#[key]
struct ParentPK {
    pub id: u32,
}

#[key]
struct Key {
    pub key: u32,
}

#[entity]
struct TransientAnnotationStruct {
    #[pk]
    id: u32,
    #[transient]
    name: Key
}

#[key]
struct ChildPK {
    #[parent]
    pub parent_pointer: ParentPK,
}

#[entity]
struct StructWithPersistedEntityField {
    #[pk]
    id: u32,
    #[column(index)]
    persisted_indexed_no_dict: i32,
}

#[entity]
struct StructWithPersistedEntityFieldWithDict {
    #[pk]
    id: u32,
    #[column(index, dictionary)]
    persisted_indexed_with_dict: i32,
}

#[entity]
pub struct FullStruct {
    #[pk]
    pub id: u32,
    #[column]
    pub amount: u32,
    #[column(index)]
    pub datum: Datum,
    #[column(index, dictionary)]
    pub address: Address,
}

#[entity]
struct MultipleOne2ManyAnnotationsStruct {
    #[pk(range)]
    id: ParentPK,
    #[one2many]
    foos: Vec<MinimalStruct>,
    #[one2many]
    bars: Vec<MinimalStruct>,
}

#[entity]
struct MissingColumnsStruct {
    #[pk]
    id: u32,
}

fn main() {
    let parent_pointer = ParentPK {id: 5};
    let child_pointer_0 = ChildPK::from_parent(parent_pointer.clone());
    let child_pointer_1 = child_pointer_0.next();
    let _ = MinimalStruct { id: child_pointer_0.clone(), persisted_no_index_no_dict: 42 };
    let _ = StructWithPersistedEntityField { id: 2, persisted_indexed_no_dict: 43 };
    let _ = StructWithPersistedEntityFieldWithDict { id: 3, persisted_indexed_with_dict: 44 };
    let _ = FullStruct { id: 4, amount: 45, datum: Datum::default(), address: Address::default() };
    let _ = MissingColumnsStruct { id: 0 };
    let _ = MultipleOne2ManyAnnotationsStruct { id: parent_pointer, foos: vec![MinimalStruct { id: child_pointer_0.clone(), persisted_no_index_no_dict: 46 }], bars: vec![MinimalStruct { id: child_pointer_1, persisted_no_index_no_dict: 47 }] };
    let _ = TransientAnnotationStruct { id: 1, name: Key { key: 48 } };
}
