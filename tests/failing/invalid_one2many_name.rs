use redbit::*;

#[root_key] struct ParentPK(u32);
#[pointer_key] struct MultipleOne2ManyPointer(ParentPK);


#[entity]
struct DoublePkStruct {
    #[pk]
    id1: ParentPK,
    foo_entities: Vec<Foo> // should be foos: Vec<Foo>
}

#[entity]
struct Foo {
    #[fk(one2many)]
    pub id: MultipleOne2ManyPointer,
    #[column]
    pub persisted_no_index_no_dict: u32,
}

fn main() {
    // If it compiles successfully, we're good.
}
