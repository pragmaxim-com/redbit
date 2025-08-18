use crate::model_v1::{Block, BlockHash, BlockHeader, BlockPointer, Height, InputRef, Transaction, TransactionPointer};
use syncer::api::*;
use redbit::*;
use std::sync::Arc;
use redbit::storage::StorageReadTx;

pub struct CardanoBlockChain {
    pub storage: Arc<Storage>,
}

impl CardanoBlockChain {
    pub fn new(storage: Arc<Storage>) -> Arc<dyn BlockChainLike<Block>> {
        let chain = CardanoBlockChain { storage };
        chain.init().expect("Failed to initialize CardanoBlockPersistence");
        Arc::new(chain)
    }

    fn populate_inputs(read_tx: &StorageReadTx, block: &mut Block) -> Result<(), ChainSyncError> {
        for tx in &mut block.transactions {
            for transient_input in tx.transient_inputs.iter_mut() {
                let tx_pointers =
                    Transaction::get_ids_by_hash(read_tx, &transient_input.tx_hash)?;

                match tx_pointers.first() {
                    Some(tx_pointer) => {
                        tx.inputs.push(InputRef {
                            id: TransactionPointer::from_parent(*tx_pointer, transient_input.index as u16),
                        });
                    }
                    None => {
                        tx.inputs.push(InputRef {
                            id: TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0)
                        })
                    }
                }
            }
        }
        Ok(())
    }
}

impl BlockChainLike<Block> for CardanoBlockChain {

    fn init(&self) -> Result<(), ChainSyncError> {
        Ok(Block::init(Arc::clone(&self.storage))?)
    }

    fn get_last_header(&self) -> Result<Option<BlockHeader>, ChainSyncError> {
        let read_tx = self.storage.begin_read()?;
        let last = BlockHeader::last(&read_tx)?;
        Ok(last)
    }

    fn get_header_by_hash(&self, hash: [u8; 32]) -> Result<Vec<BlockHeader>, ChainSyncError> {
        let read_tx = self.storage.begin_read()?;
        let header = BlockHeader::get_by_hash(&read_tx, &BlockHash(hash))?;
        Ok(header)
    }

    fn store_blocks(&self, mut blocks: Vec<Block>) -> Result<(), ChainSyncError> {
        for block in &mut blocks {
            let read_tx = self.storage.begin_read()?;
            Self::populate_inputs(&read_tx, block)?;
            Block::store_and_commit(Arc::clone(&self.storage), block)?;
        }
        Ok(())
    }

    fn update_blocks(&self, mut blocks: Vec<Block>) -> Result<(), ChainSyncError> {
        let write_tx = self.storage.begin_write()?;
        for block in &mut blocks {
            Block::delete(&write_tx, &block.height)?;
        }
        write_tx.commit()?;
        self.store_blocks(blocks)?;
        Ok(())
    }
}
