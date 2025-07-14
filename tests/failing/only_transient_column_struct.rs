#[entity]
struct TransientAnnotationStruct {
    #[pk]
    id: ParentPK,
    #[column(transient)]
    name: String
}
