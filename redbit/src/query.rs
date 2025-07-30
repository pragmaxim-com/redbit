use crate::{Deserialize, IntoParams, Serialize};

#[derive(IntoParams, Serialize, Deserialize, Default)]
pub struct TakeQuery {
    #[param(required = true, example = 10)]
    pub take: usize,
}

impl TakeQuery {
    pub fn sample() -> TakeQuery {
        TakeQuery { take: 2 }
    }
}

#[derive(IntoParams, Serialize, Deserialize, Default)]
pub struct TailQuery {
    #[param(required = true, example = 10)]
    pub tail: usize,
}

impl TailQuery {
    pub fn sample() -> TailQuery {
        TailQuery { tail: 2 }
    }
}
