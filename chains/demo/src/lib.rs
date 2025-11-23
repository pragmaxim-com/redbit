#![feature(test)]
extern crate test;

pub mod block_provider;
pub mod model_v1;
pub mod routes;
pub mod hook;
pub mod manual_runtime;
pub mod manual_chain;

use crate::model_v1::chrono::DateTime;
use std::fmt::Display;
use crate::model_v1::{BlockHash, Timestamp};

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
