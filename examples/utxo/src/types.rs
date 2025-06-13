use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use derive_more::{Add, From};
use utoipa::ToSchema;

#[derive(Debug, ToSchema, Default, Serialize, Deserialize, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Add, From)]
pub struct Height(pub u32);

impl std::ops::Add<u32> for Height {
    type Output = Self;

    fn add(self, other: u32) -> Self {
        Height(self.0 + other)
    }
}

#[derive(Debug, ToSchema, Default, Serialize, Deserialize, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
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
