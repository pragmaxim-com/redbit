use redbit::*;

#[derive(Redbit)]
struct MissingFieldAnnotationStruct {
    #[pk]
    id: u32,
    name: String,
}

fn main() {
    // If it compiles successfully, we're good.
}
