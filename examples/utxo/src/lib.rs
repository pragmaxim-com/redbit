pub mod data;
pub mod demo;

pub use data::*;
pub use redbit::*;

pub type Timestamp = u32;
pub type Height = u32;
pub type Amount = u64;
pub type Nonce = u32;

#[indexed_column] pub struct Hash(pub String);
#[indexed_column] pub struct Address(pub String);
#[indexed_column] pub struct Datum(pub String);
#[indexed_column] pub struct PolicyId(pub String);
#[indexed_column] pub struct AssetName(pub String);

#[entity]
pub struct Block {
    #[pk(range)]
    pub id: BlockPointer,
    #[one2one]
    pub header: BlockHeader,
    #[one2many]
    pub transactions: Vec<Transaction>,
}

#[entity]
pub struct BlockHeader {
    #[fk(one2one, range)]
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

#[entity]
pub struct Transaction {
    #[fk(one2many, range)]
    pub id: TxPointer,
    #[column(index)]
    pub hash: Hash,
    #[one2many]
    pub utxos: Vec<Utxo>,
    #[one2many]
    pub inputs: Vec<InputRef>,
}

#[entity]
pub struct Utxo {
    #[fk(one2many, range)]
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

#[entity]
pub struct InputRef {
    #[fk(one2many, range)]
    pub id: InputPointer,
}

#[entity]
pub struct Asset {
    #[fk(one2many, range)]
    pub id: AssetPointer,
    #[column]
    pub amount: Amount,
    #[column(index, dictionary)]
    pub name: AssetName,
    #[column(index, dictionary)]
    pub policy_id: PolicyId,
}

#[key]
pub struct BlockPointer {
    pub height: Height,
}

#[key(u16)]
pub struct TxPointer {
    #[parent]
    pub block_pointer: BlockPointer,
}

#[key(u8)]
pub struct UtxoPointer {
    #[parent]
    pub tx_pointer: TxPointer,
}

#[key(u8)]
pub struct InputPointer {
    #[parent]
    pub tx_pointer: TxPointer,
}

#[key(u8)]
pub struct AssetPointer {
    #[parent]
    pub utxo_pointer: UtxoPointer,
}
