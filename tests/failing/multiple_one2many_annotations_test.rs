use redbit::*;

#[derive(Entity)]
struct MultipleOne2ManyAnnotationsStruct {
    #[pk]
    id: u32,
    #[one2many]
    foos: Vec<String>,
    #[one2many]
    bars: Vec<String>,
}

fn main() {
    // If it compiles successfully, we're good.
}
