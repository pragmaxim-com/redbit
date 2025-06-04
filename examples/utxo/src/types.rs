use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::ops::Add;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct Height(pub u32);

impl Default for Height {
    fn default() -> Self {
        Height(0)
    }
}
impl Add<u32> for Height {
    type Output = Self;

    fn add(self, other: u32) -> Self {
        Height(self.0 + other)
    }
}
impl Add for Height {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Height(self.0 + other.0)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub struct Timestamp(pub u32);

pub type Amount = u64;
pub type Nonce = u32;

pub type TxIndex = u16;
pub type UtxoIndex = u16;
pub type AssetIndex = u16;
pub type Datum = String;
pub type Address = String;
pub type AssetName = String;
pub type PolicyId = String;
pub type Hash = String;
