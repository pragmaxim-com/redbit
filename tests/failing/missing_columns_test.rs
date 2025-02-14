use redbit::*;

#[derive(Redbit)]
struct MissingColumnsStruct {
    #[pk]
    id: u32,
}

fn main() {
    // If it compiles successfully, we're good.
}
