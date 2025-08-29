#![feature(test)]
extern crate test;

pub mod block_provider;
pub mod ergo_client;
pub mod config;
pub mod model_v1;
pub mod codec;
pub mod block_chain;

use chain::ChainError;
use std::error::Error;
use num_enum::{IntoPrimitive, TryFromPrimitive};

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
