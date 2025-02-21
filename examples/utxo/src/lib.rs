mod codec;

pub use redbit::*;
use std::fmt::Debug;

pub type Amount = u64;
pub type Timestamp = u64;
pub type Height = u32;
pub type TxIndex = u16;
pub type UtxoIndex = u16;
pub type Datum = String;
pub type Address = String;
pub type Hash = String;

#[derive(Entity, Debug, Clone, PartialEq, Eq)]
pub struct Block {
    #[pk(range)]
    pub id: BlockPointer,
    #[column(index)]
    pub hash: Hash,
    #[column(index, range)]
    pub timestamp: Timestamp,
    pub transactions: Vec<Transaction>,
}

#[derive(Entity, Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    #[pk(range)]
    pub id: TxPointer,
    #[column(index)]
    pub hash: Hash,
    pub utxos: Vec<Utxo>,
}

#[derive(Entity, Debug, Clone, PartialEq, Eq)]
pub struct Utxo {
    #[pk(range)]
    pub id: UtxoPointer,
    #[column]
    pub amount: Amount,
    #[column(index)]
    pub datum: Datum,
    #[column(index, dictionary)]
    pub address: Address,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UtxoPointer {
    pub tx_pointer: TxPointer,
    pub utxo_index: UtxoIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxPointer {
    pub block_pointer: BlockPointer,
    pub tx_index: TxIndex,
}


impl FkRange<UtxoPointer> for TxPointer {
    fn fk_range(&self) -> (UtxoPointer, UtxoPointer) {
        (
            UtxoPointer {
                tx_pointer: self.clone(),
                utxo_index: TxIndex::MIN,
            },
            UtxoPointer {
                tx_pointer: self.clone(),
                utxo_index: TxIndex::MAX,
            }
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockPointer {
    pub height: Height,
}

impl FkRange<TxPointer> for BlockPointer {
    fn fk_range(&self) -> (TxPointer, TxPointer) {
        (
            TxPointer {
                block_pointer: self.clone(),
                tx_index: TxIndex::MIN,
            },
            TxPointer {
                block_pointer: self.clone(),
                tx_index: TxIndex::MAX,
            }
        )
    }
}
