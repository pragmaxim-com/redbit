#![feature(test)]
extern crate test;

pub mod block_provider;
pub mod rest_client;
pub mod config;
pub mod model_v1;
pub mod codec;
pub mod block_chain;

use bitcoin::block::Bip34Error;
use chain::ChainError;

#[derive(Debug, thiserror::Error)]
pub enum ExplorerError {
    #[error("Height decoding error: {0}")]
    Bip34(#[from] Bip34Error),

    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Custom error: {0}")]
    Custom(String),
}

impl From<ExplorerError> for ChainError {
    fn from(err: ExplorerError) -> Self {
        ChainError::new(&err.to_string())
    }
}
