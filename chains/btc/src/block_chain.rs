use std::fmt::Display;
use chrono::DateTime;
use chain::{BlockChainLike, ChainError};
use crate::model_v1::*;


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

pub struct BlockChain {
    pub storage: Arc<Storage>,
}

impl BlockChain {
    pub fn new(storage: Arc<Storage>) -> Arc<dyn BlockChainLike<Block>> {
        Arc::new(BlockChain { storage })
    }

    pub(crate) fn resolve_tx_inputs(tx_context: &TransactionWriteTxContext, transactions: &mut [Transaction]) -> Result<(), ChainError> {
        for tx in transactions {
            for transient_input in tx.temp_input_refs.iter_mut() {
                match tx_context.transaction_hash_index.get(&transient_input.tx_hash)?.next() {
                    Some(Ok(tx_pointer)) => tx.inputs.push(Input { id: TransactionPointer::from_parent(tx_pointer.value(), transient_input.index as u16) }),
                    _ => tx.inputs.push(Input { id: TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0) }),
                }
            }
        }
        Ok(())
    }
}
