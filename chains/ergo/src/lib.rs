#![feature(test)]
extern crate test;

pub mod block_provider;
pub mod ergo_client;
pub mod model_v1;
pub mod codec;
pub mod hook;

use std::error::Error;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use crate::model_v1::chrono::DateTime;
use std::fmt::Display;
use chain::err::ChainError;
use chain::settings::Parallelism;
use crate::model_v1::{BlockHash, Deserialize, Timestamp};

#[derive(Debug, Deserialize, Clone)]
pub struct ErgoConfig {
    pub api_host: String,
    pub api_key: String,
    pub fetching_parallelism: Parallelism,
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
    #[error("Reqwest error: {source}{}", source.source().map(|e| format!(": {}", e)).unwrap_or_default())]
    Reqwest {
        #[from]
        source: reqwest::Error,
    },

    #[error("Url parsing error: {0}")]
    Url(#[from] url::ParseError),

    #[error("Invalid http header value : {0}")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),

    #[error("Custom error: {0}")]
    Custom(String),
}

impl From<ExplorerError> for ChainError {
    fn from(err: ExplorerError) -> Self {
        ChainError::new(&err.to_string())
    }
}
