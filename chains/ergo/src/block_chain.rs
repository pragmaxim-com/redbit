use chain::{BlockChainLike, ChainError};
use crate::model_v1::*;

pub struct BlockChain {
    pub storage: Arc<Storage>,
}

impl BlockChain {
    pub fn new(storage: Arc<Storage>) -> Arc<dyn BlockChainLike<Block>> {
        Arc::new(BlockChain { storage })
    }

    pub(crate) fn resolve_tx_inputs(&self, tx_context: &BlockReadTxContext, block: &mut Block) -> Result<(), ChainError> {
        for tx in &mut block.transactions {
            for box_id in tx.transient_inputs.iter_mut() {
                let utxo_pointers = Utxo::get_ids_by_box_id(&tx_context.transactions.utxos, box_id).expect("Failed to get Utxo by ErgoBox");
                match utxo_pointers.first() {
                    Some(utxo_pointer) => {
                        tx.inputs.push(InputRef { id: TransactionPointer::from_parent(utxo_pointer.parent, utxo_pointer.index()) })
                    }
                    None => tx.inputs.push(InputRef { id: TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0) }),
                }
            }
        }
        Ok(())
    }

}
