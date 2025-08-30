use redbit::*;

#[entity]
struct MissingColumnsStruct {
    #[pk]
    id: ParentPK,
}
