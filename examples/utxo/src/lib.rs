pub mod data;
pub mod types;
pub mod demo;
pub use data::*;
pub use redbit::*;
pub use types::*;

use derive_more::From;
use serde::{Deserialize, Serialize};

#[derive(Entity, Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Block {
    #[pk(range)]
    pub id: BlockPointer,
    #[one2one]
    pub header: BlockHeader,
    #[one2many]
    pub transactions: Vec<Transaction>,
}

#[derive(Entity, Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
    pub nonce: Nonce,
}

#[derive(Entity, Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    #[pk(range)]
    pub id: TxPointer,
    #[column(index)]
    pub hash: Hash,
    #[one2many]
    pub utxos: Vec<Utxo>,
    #[one2many]
    pub inputs: Vec<InputRef>,
}

#[derive(Entity, Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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

#[derive(Entity, Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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

#[derive(PK, Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, From)]
pub struct BlockPointer {
    pub height: Height,
}

#[derive(PK, Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct TxPointer {
    #[parent]
    pub block_pointer: BlockPointer,
    pub tx_index: TxIndex,
}

#[derive(PK, Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct UtxoPointer {
    #[parent]
    pub tx_pointer: TxPointer,
    pub utxo_index: UtxoIndex,
}

#[derive(PK, Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct InputPointer {
    #[parent]
    pub tx_pointer: TxPointer,
    pub utxo_index: UtxoIndex,
}

#[derive(Entity, Default, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InputRef {
    #[pk(range)]
    pub id: InputPointer,
}

#[derive(PK, Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetPointer {
    #[parent]
    pub utxo_pointer: UtxoPointer,
    pub asset_index: AssetIndex,
}
