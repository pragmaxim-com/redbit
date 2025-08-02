#![feature(test)]
extern crate test;

pub mod data;
pub mod demo;
pub mod routes;

pub use data::*;
pub use redbit::*;

#[root_key] pub struct Height(pub u32);

#[pointer_key(u16)] pub struct BlockPointer(Height);
#[pointer_key(u16)] pub struct TransactionPointer(BlockPointer);
#[pointer_key(u8)] pub struct UtxoPointer(TransactionPointer);

#[column] pub struct Hash(pub String);
#[column("btc_addr")] pub struct BtcAddress(pub Vec<u8>);
#[column("cardano_addr")] pub struct CardanoAddress(pub Vec<u8>);
#[column("hex")] pub struct SomeFixedBytes(pub [u8; 32]);
#[column] pub struct AssetName(pub String);
#[column] pub struct Time(pub chrono::DateTime<chrono::Utc>);
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
    pub mining_time: Time, // just to demonstrate a different type
    #[column]
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
    #[column(index)]
    pub fixed_bytes: SomeFixedBytes,
    #[column(dictionary)]
    pub btc_address: BtcAddress,
    #[column(dictionary)]
    pub cardano_address: CardanoAddress,
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
    #[column(dictionary)]
    pub name: AssetName,
}
