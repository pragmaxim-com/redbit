pub use redbit::*;
pub use chain::*;

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
pub struct InputRef {
    pub tx_hash: TxHash,
    pub index: u16,
}

#[entity]
pub struct Block {
    #[pk(db_cache = 1)]
    pub height: Height,
    pub header: Header,
    pub transactions: Vec<Transaction>,
}

#[entity]
pub struct Header {
    #[fk(one2one, db_cache = 1)]
    pub height: Height,
    #[column(index, db_cache = 1)]
    pub hash: BlockHash,
    #[column(index, db_cache = 1)]
    pub prev_hash: BlockHash,
    #[column(range, db_cache = 1)]
    pub timestamp: Timestamp,
    #[column(index, db_cache = 1)]
    pub merkle_root: MerkleRoot,
    #[column(transient)]
    pub weight: Weight,
}

#[entity]
pub struct Transaction {
    #[fk(one2many, db_cache = 2)]
    pub id: BlockPointer,
    #[column(index, used, shards = 4, db_cache = 42, lru_cache = 16)]
    pub hash: TxHash,
    pub utxos: Vec<Utxo>,
    #[write_from_using(input_refs, hash)] // implement custom write_from_using function, see hook.rs
    pub inputs: Vec<Input>,
    #[column(transient)]
    pub input_refs: Vec<InputRef>,
    #[column(transient(read_from(inputs::utxo_pointer)))]
    pub input_utxos: Vec<Utxo>,
}

#[entity]
pub struct Utxo {
    #[fk(one2many, db_cache = 3)]
    pub id: TransactionPointer,
    #[column(db_cache = 3)]
    pub amount: u64,
    #[column(dictionary, shards = 5, db_cache = 44, lru_cache = 9)]
    pub script_hash: ScriptHash,
    #[column(dictionary, shards = 5, db_cache = 36, lru_cache = 7)]
    pub address: Address,
}

#[entity]
pub struct Input {
    #[fk(one2many, db_cache = 3)]
    pub id: TransactionPointer,
    #[column(pointer, db_cache = 3)]
    pub utxo_pointer: TransactionPointer,
}
