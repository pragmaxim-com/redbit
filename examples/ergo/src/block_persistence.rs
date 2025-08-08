use crate::model::{Block, BlockHash, BlockHeader, BlockPointer, Height, InputRef, TransactionPointer, Utxo};
use syncer::api::*;
use redbit::redb::ReadTransaction;
use redbit::*;
use std::sync::Arc;

pub struct ErgoBlockPersistence {
    pub db: Arc<Database>,
}

impl ErgoBlockPersistence {
    fn populate_inputs(read_tx: &ReadTransaction, block: &mut Block) -> Result<(), ChainSyncError> {
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
    fn get_last_header(&self) -> Result<Option<BlockHeader>, ChainSyncError> {
        let read_tx = self.db.begin_read()?;
        let last = BlockHeader::last(&read_tx)?;
        Ok(last)
    }

    fn get_header_by_hash(&self, hash: [u8; 32]) -> Result<Vec<BlockHeader>, ChainSyncError> {
        let read_tx = self.db.begin_read()?;
        let header = BlockHeader::get_by_hash(&read_tx, &BlockHash(hash))?;
        Ok(header)
    }

    fn store_blocks(&self, mut blocks: Vec<Block>) -> Result<(), ChainSyncError> {
        for block in &mut blocks {
            let read_tx = self.db.begin_read()?;
            Self::populate_inputs(&read_tx, block)?;
            Block::store_and_commit(&self.db, block)?;
        }
        Ok(())
    }

    fn update_blocks(&self, mut blocks: Vec<Block>) -> Result<(), ChainSyncError> {
        let write_tx = self.db.begin_write()?;
        for block in &mut blocks {
            Block::delete(&write_tx, &block.id)?;
        }
        write_tx.commit()?;
        self.store_blocks(blocks)?;
        Ok(())
    }
}
