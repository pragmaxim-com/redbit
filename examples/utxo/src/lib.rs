pub use redbit::*;
use std::fmt::Debug;

type UtxoId = [u8; 32];
type Amount = u64;
type Datum = String;
type Address = String;

#[derive(Redbit, Debug, Default)]
pub struct Utxo {
    #[pk]
    pub id: UtxoId,
    #[column]
    pub amount: Amount,
    #[column(index)]
    pub datum: Datum,
    #[column(index, dictionary)]
    pub address: Address,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UtxoPointer {
    pub block_height: u32,
    pub tx_index: u16,
    pub utxo_index: u16,
}

impl From<UtxoPointer> for UtxoId {
    fn from(id: UtxoPointer) -> Self {
        let mut pk = [0u8; 32];
        pk[0..4].copy_from_slice(&id.block_height.to_be_bytes());
        pk[4..6].copy_from_slice(&id.tx_index.to_be_bytes());
        pk[6..8].copy_from_slice(&id.utxo_index.to_be_bytes());
        pk
    }
}