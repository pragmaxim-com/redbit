pub mod data;
pub mod demo;

pub use data::*;
pub use redbit::*;

#[root_key] pub struct Height(pub u32);

#[pointer_key(u16)] pub struct TxPointer(Height);
#[pointer_key(u16)] pub struct UtxoPointer(TxPointer);
#[pointer_key(u16)] pub struct InputPointer(TxPointer);
#[pointer_key(u8)] pub struct AssetPointer(UtxoPointer);

#[index] pub struct Hash(pub String);
#[index] pub struct Address(pub [u8; 32]);
#[index] pub struct PolicyId(pub String);
#[index] pub struct Datum(pub Vec<u8>);
#[index] pub struct AssetName(pub String);

#[index]
#[derive(Copy, Hash)]
pub struct Timestamp(pub u32);

#[entity]
pub struct Block {
    #[pk(range)]
    pub id: Height,
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    #[column(transient)]
    pub weight: u32,
}

#[entity]
pub struct BlockHeader {
    #[fk(one2one, range)]
    pub id: Height,
    #[column(index)]
    pub hash: Hash,
    #[column(range)]
    pub timestamp: Timestamp,
    #[column(index)]
    pub merkle_root: Hash,
    #[column]
    pub nonce: u64,
}

#[entity]
pub struct Transaction {
    #[fk(one2many, range)]
    pub id: TxPointer,
    #[column(index)]
    pub hash: Hash,
    pub utxos: Vec<Utxo>,
    pub inputs: Vec<InputRef>,
}

#[entity]
pub struct Utxo {
    #[fk(one2many, range)]
    pub id: UtxoPointer,
    #[column]
    pub amount: u64,
    #[column(index)]
    pub datum: Datum,
    #[column(dictionary)]
    pub address: Address,
    pub assets: Vec<Asset>,
    pub tree: Option<Tree>,
}

#[entity]
pub struct Tree {
    #[fk(one2opt, range)]
    pub id: UtxoPointer,
    #[column(index)]
    pub hash: Hash,
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
    pub amount: u64,
    #[column(dictionary)]
    pub name: AssetName,
    #[column(dictionary)]
    pub policy_id: PolicyId,
}
