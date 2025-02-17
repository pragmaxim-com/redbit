use redbit::*;

#[derive(Redbit)]
struct MultipleAnnotationsStruct {
    #[pk]
    id: u32,
    #[column]
    #[column]
    name: String,
}

fn main() {
    // If it compiles successfully, we're good.
}
