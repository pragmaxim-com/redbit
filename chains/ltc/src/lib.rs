#![feature(test)]
extern crate test;

pub mod block_provider;
pub mod rest_client;
pub mod model_v1;
pub mod codec;
pub mod hook;

use crate::model_v1::chrono::DateTime;
use crate::model_v1::{BlockHash, Deserialize, Timestamp};
use chain::err::ChainError;
use chain::settings::Parallelism;
use std::fmt::Display;

#[derive(Debug, Deserialize, Clone)]
pub struct LitecoinConfig {
    pub api_host: String,
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

#[derive(Debug, thiserror::Error)]
pub enum ExplorerError {
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Custom error: {0}")]
    Custom(String),
}

impl From<ExplorerError> for ChainError {
    fn from(err: ExplorerError) -> Self { ChainError::new(&err.to_string()) }
}
