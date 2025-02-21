use redbit::*;

#[derive(Entity)]
struct MissingColumnsStruct {
    #[pk]
    id: u32,
}

fn main() {
    // If it compiles successfully, we're good.
}
