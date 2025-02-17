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

#[derive(Redbit, Debug, Clone, PartialEq, Eq)]
pub struct Block {
    #[pk]
    pub hash: Hash,
    #[column]
    pub timestamp: Timestamp,
    #[column(index, range)]
    pub height: Height,
}

#[derive(Redbit, Debug, Clone, PartialEq, Eq)]
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
    pub block_height: Height,
    pub tx_index: TxIndex,
    pub utxo_index: UtxoIndex,
}
