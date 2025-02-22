mod data;

pub use redbit::*;
pub use data::*;

use std::fmt::Debug;
use serde::{Deserialize, Serialize};

pub type Amount = u64;
pub type Timestamp = u64;
pub type Height = u32;
pub type TxIndex = u16;
pub type UtxoIndex = u16;
pub type AssetIndex = u16;
pub type Datum = String;
pub type Address = String;
pub type AssetName = String;
pub type PolicyId = String;
pub type Hash = String;

#[derive(Entity, Debug, Clone, PartialEq, Eq)]
pub struct Block {
    #[pk(range)]
    pub id: BlockPointer,
    #[one2one]
    pub header: BlockHeader,
    #[one2many]
    pub transactions: Vec<Transaction>,
}

#[derive(Entity, Debug, Clone, PartialEq, Eq)]
pub struct BlockHeader {
    #[pk(range)]
    pub id: BlockPointer,
    #[column(index)]
    pub hash: Hash,
    #[column(index, range)]
    pub timestamp: Timestamp,
    #[column(index)]
    pub merkle_root: Hash,
    #[column]
    pub nonce: u32,
}

#[derive(Entity, Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    #[pk(range)]
    pub id: TxPointer,
    #[column(index)]
    pub hash: Hash,
    #[one2many]
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
    #[one2many]
    pub assets: Vec<Asset>,
}

#[derive(Entity, Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    #[pk(range)]
    pub id: AssetPointer,
    #[column]
    pub amount: Amount,
    #[column(index, dictionary)]
    pub name: AssetName,
    #[column(index, dictionary)]
    pub policy_id: PolicyId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockPointer {
    pub height: Height,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct TxPointer {
    pub block_pointer: BlockPointer,
    pub tx_index: TxIndex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct UtxoPointer {
    pub tx_pointer: TxPointer,
    pub utxo_index: UtxoIndex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetPointer {
    pub utxo_pointer: UtxoPointer,
    pub asset_index: AssetIndex,
}

impl PK<TxPointer> for BlockPointer {
    fn fk_range(&self) -> (TxPointer, TxPointer) {
        (TxPointer { block_pointer: self.clone(), tx_index: TxIndex::MIN }, TxPointer { block_pointer: self.clone(), tx_index: TxIndex::MAX })
    }
}

impl PK<UtxoPointer> for TxPointer {
    fn fk_range(&self) -> (UtxoPointer, UtxoPointer) {
        (UtxoPointer { tx_pointer: self.clone(), utxo_index: UtxoIndex::MIN }, UtxoPointer { tx_pointer: self.clone(), utxo_index: UtxoIndex::MAX })
    }
}

impl PK<AssetPointer> for UtxoPointer {
    fn fk_range(&self) -> (AssetPointer, AssetPointer) {
        (
            AssetPointer { utxo_pointer: self.clone(), asset_index: AssetIndex::MIN },
            AssetPointer { utxo_pointer: self.clone(), asset_index: AssetIndex::MAX },
        )
    }
}
