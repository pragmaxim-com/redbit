pub use redbit::*;
pub use chain::*;
use crate::block_chain::BlockChain;

// feel free to add custom #[derive(Foo, Bar)] attributes to your types, they will get merged with the ones from redbit

#[root_key] pub struct Height(pub u32);

#[pointer_key(u16)] pub struct BlockPointer(Height);
#[pointer_key(u16)] pub struct TransactionPointer(BlockPointer);
#[pointer_key(u8)] pub struct UtxoPointer(TransactionPointer);

#[column("hex")] pub struct BlockHash(pub [u8; 32]);
#[column("hex")] pub struct MerkleRoot(pub [u8; 32]);
#[column("hex")] pub struct TxHash(pub [u8; 32]);
#[column("hex")] pub struct ScriptHash(pub Vec<u8>);
#[column] pub struct Timestamp(pub u32);
#[column] pub struct Weight(pub u32);

#[column("crate::codec::BaseOrBech")]
pub struct Address(pub Vec<u8>);

#[column]
pub struct TempInputRef {
    pub tx_hash: TxHash,
    pub index: u32,
}

#[entity]
pub struct Block {
    #[pk]
    pub height: Height,
    pub header: Header,
    pub transactions: Vec<Transaction>,
}

#[entity]
pub struct Header {
    #[fk(one2one)]
    pub height: Height,
    #[column(index)]
    pub hash: BlockHash,
    #[column(index)]
    pub prev_hash: BlockHash,
    #[column(range)]
    pub timestamp: Timestamp,
    #[column(index)]
    pub merkle_root: MerkleRoot,
    #[column(transient)]
    pub weight: Weight,
}

#[entity]
pub struct Transaction {
    #[fk(one2many)]
    pub id: BlockPointer,
    #[column(index)]
    pub hash: TxHash,
    pub utxos: Vec<Utxo>,
    pub inputs: Vec<Input>,
    #[column(transient)]
    pub temp_input_refs: Vec<TempInputRef>,
}

#[entity]
pub struct Utxo {
    #[fk(one2many)]
    pub id: TransactionPointer,
    #[column]
    pub amount: u64,
    #[column(dictionary)]
    pub script_hash: ScriptHash,
    #[column(dictionary)]
    pub address: Address,
}

#[entity]
pub struct Input {
    #[fk(one2many)]
    pub id: TransactionPointer,
}
