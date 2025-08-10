#![feature(test)]
extern crate test;

pub mod data;
pub mod run;
pub mod routes;

pub use data::*;
pub use redbit::*;

#[root_key] pub struct Height(pub u32);

#[pointer_key(u16)] pub struct BlockPointer(Height);
#[pointer_key(u16)] pub struct TransactionPointer(BlockPointer);
#[pointer_key(u8)] pub struct UtxoPointer(TransactionPointer);

// #[column] pub struct Time(pub chrono::DateTime<chrono::Utc>);

#[column("hex")] pub struct Hash(pub [u8; 32]);
#[column("base64")] pub struct Address(pub Vec<u8>);
#[column("utf-8")] pub struct AssetName(pub Vec<u8>); // String is supported but this is more efficient
#[column] pub struct Duration(pub std::time::Duration);
#[column]
#[derive(Copy, Hash)]
pub struct Timestamp(pub u32);

#[column]
pub struct TempInputRef {
    tx_hash: Hash,
    index: u32,
}

#[entity]
pub struct Block {
    #[pk]
    pub height: Height,
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    #[column(transient)]
    pub weight: u32,
}

#[entity]
pub struct BlockHeader {
    #[fk(one2one)]
    pub height: Height,
    #[column(index)]
    pub hash: Hash,
    #[column(range)]
    pub timestamp: Timestamp,
    #[column(range)]
    pub duration: Duration,
    #[column]
    pub nonce: u64,
}

#[entity]
pub struct Transaction {
    #[fk(one2many)]
    pub id: BlockPointer,
    #[column(index)]
    pub hash: Hash,
    pub utxos: Vec<Utxo>,
    pub input: Option<InputRef>, // intentionally Option to demonstrate it is possible
    #[column(transient)]
    pub transient_inputs: Vec<TempInputRef>,
}

#[entity]
pub struct Utxo {
    #[fk(one2many)]
    pub id: TransactionPointer,
    #[column]
    pub amount: u64,
    #[column(dictionary(cache = 1000000))]
    pub address: Address,
    pub assets: Vec<Asset>,
}

#[entity]
pub struct InputRef {
    #[fk(one2opt)]
    pub id: BlockPointer,
    #[column(index)]
    pub hash: Hash, // just dummy values
}

#[entity]
pub struct Asset {
    #[fk(one2many)]
    pub id: UtxoPointer,
    #[column]
    pub amount: u64,
    #[column(dictionary(cache = 1000000))]
    pub name: AssetName,
}
