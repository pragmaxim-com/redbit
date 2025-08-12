use crate::model_v1::{Block, BlockHash, BlockHeader, BlockPointer, Height, InputRef, TransactionPointer, Utxo};
use syncer::api::*;
use redbit::*;
use std::sync::Arc;
use redbit::storage::{Storage, StorageReadTx};

pub struct ErgoBlockPersistence {
    pub storage: Arc<Storage>,
}

impl ErgoBlockPersistence {
    pub fn new(storage: Arc<Storage>) -> Self {
        let persistence = ErgoBlockPersistence { storage };
        persistence.init().expect("Failed to initialize ErgoBlockPersistence");
        persistence
    }

    fn populate_inputs(read_tx: &StorageReadTx, block: &mut Block) -> Result<(), ChainSyncError> {
        for tx in &mut block.transactions {
            for box_id in tx.transient_inputs.iter_mut() {
                let utxo_pointers = Utxo::get_ids_by_box_id(read_tx, &box_id).expect("Failed to get Utxo by ErgoBox");
                match utxo_pointers.first() {
                    Some(utxo_pointer) => {
                        tx.inputs.push(InputRef { id: TransactionPointer::from_parent(utxo_pointer.parent.clone(), utxo_pointer.index()) })
                    }
                    None => tx.inputs.push(InputRef { id: TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0) }),
                }
            }
        }
        Ok(())
    }
}

impl BlockPersistence<Block> for ErgoBlockPersistence {
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
