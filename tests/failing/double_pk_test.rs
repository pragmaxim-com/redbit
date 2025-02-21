use redbit::*;

#[derive(Entity)]
struct DoublePkStruct {
    #[pk]
    id1: u32,
    #[pk]
    id2: u32,
}

fn main() {
    // If it compiles successfully, we're good.
}
