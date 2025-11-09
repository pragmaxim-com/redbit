#![feature(test)]
extern crate test;

pub mod block_provider;
pub mod cardano_client;
pub mod model_v1;
pub mod codec;
pub mod hook;

use crate::model_v1::chrono::DateTime;
use crate::model_v1::{BlockHash, Deserialize, Timestamp};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallas::network::miniprotocols::{blockfetch, chainsync, localstate};
use std::fmt::Display;
use chain::err::ChainError;

#[derive(Debug, Deserialize, Clone)]
pub struct CardanoConfig {
    pub api_host: String,
    pub socket_path: String,
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let datetime = DateTime::from_timestamp(self.0 as i64, 0).unwrap();
        write!(f, "{}", datetime.format("%Y-%m-%d %H:%M:%S"))
    }
}

impl Display for BlockHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = [0u8; 12];
        hex::encode_to_slice(&self.0[..6], &mut buf).map_err(|_| std::fmt::Error)?;
        write!(f, "{}", unsafe { std::str::from_utf8_unchecked(&buf) })
    }
}

#[derive(Clone, Copy, Debug, IntoPrimitive, PartialEq, TryFromPrimitive, )]
#[repr(u8)]
pub enum AssetType {
    Mint = 0,
    Transfer = 1,
    Burn = 2,
}

#[derive(Debug, thiserror::Error)]
pub enum ExplorerError {
    #[error("Cardano chain sync error: {0}")]
    ChainSyncError(#[from] chainsync::ClientError),

    #[error("Cardano block fetch error: {0}")]
    BlockFetchError(#[from] blockfetch::ClientError),

    #[error("Cardano local state error: {0}")]
    LocalStateError(#[from] localstate::ClientError),

    #[error("Cardano pallas traverse error: {0}")]
    PallasTraverseError(#[from] pallas_traverse::Error),

    #[error("Custom error: {0}")]
    Custom(String),
}

impl From<ExplorerError> for ChainError {
    fn from(err: ExplorerError) -> Self {
        ChainError::new(&err.to_string())
    }
}
