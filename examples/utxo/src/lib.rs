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
#[index] pub struct Datum(pub String);
#[index] pub struct PolicyId(pub String);
#[index] pub struct AssetName(pub String);

#[index]
#[derive(Copy, Hash)]
pub struct Timestamp(pub u32);

#[entity]
pub struct Block {
    #[pk(range)]
    pub id: Height,
    #[one2one]
    pub header: BlockHeader,
    #[one2many]
    pub transactions: Vec<Transaction>,
}

#[entity]
pub struct BlockHeader {
    #[fk(one2one, range)]
    pub id: Height,
    #[column(index)]
    pub hash: Hash,
    #[column(index, range)]
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
    pub amount: u64,
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
    pub amount: u64,
    #[column(index, dictionary)]
    pub name: AssetName,
    #[column(index, dictionary)]
    pub policy_id: PolicyId,
}
