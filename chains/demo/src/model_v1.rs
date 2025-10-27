pub use redbit::*;
pub use chain::*;

// feel free to add custom #[derive(Foo, Bar)] attributes to your types, they will get merged with the ones from redbit

#[root_key] pub struct Height(pub u32);

#[pointer_key(u16)] pub struct BlockPointer(Height);
#[pointer_key(u16)] pub struct TransactionPointer(BlockPointer);
#[pointer_key(u16)] pub struct UtxoPointer(TransactionPointer);

// #[column] pub struct Time(pub chrono::DateTime<chrono::Utc>);

#[column("hex")] pub struct BlockHash(pub [u8; 32]);
#[column("hex")] pub struct TxHash(pub [u8; 32]);
#[column("base64")] pub struct Address(pub Vec<u8>);
#[column("utf-8")] pub struct AssetName(pub Vec<u8>); // String is supported but this is more efficient
#[column] pub struct Duration(pub std::time::Duration);
#[column] pub struct Weight(pub u32);

#[column] pub struct Timestamp(pub u32);

#[column]
pub struct InputRef {
    pub tx_hash: TxHash,
    pub index: u16,
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
    #[column(range)]
    pub duration: Duration,
    #[column]
    pub nonce: u64,
    #[column(transient)]
    pub weight: Weight,
}

#[entity]
pub struct Transaction {
    #[fk(one2many)]
    pub id: BlockPointer,
    #[column(index, used, shards = 3, db_cache = 4, lru_cache = 2)]
    pub hash: TxHash,
    pub utxos: Vec<Utxo>,
    #[write_from_using(input_refs, hash)] // implement custom write_from_using function, see hook.rs
    pub inputs: Vec<Input>,
    pub maybe: Option<MaybeValue>, // just to demonstrate option is possible
    #[column(transient)]
    pub input_refs: Vec<InputRef>,
    #[column(transient(read_from(inputs::utxo_pointer)))] // this field is loaded when read from inputs.utxo_pointer
    pub input_utxos: Vec<Utxo>,
}

#[entity]
pub struct Utxo {
    #[fk(one2many, db_cache = 2)]
    pub id: TransactionPointer,
    #[column(shards = 3)]
    pub amount: u64,
    #[column(dictionary, shards = 4, db_cache = 10, lru_cache = 2)]
    pub address: Address,
    pub assets: Vec<Asset>,
}

#[entity]
pub struct Input {
    #[fk(one2many, db_cache = 1)]
    pub id: TransactionPointer,
    #[column(pointer, db_cache = 1, shards = 2)]
    pub utxo_pointer: TransactionPointer,
}

#[entity]
pub struct MaybeValue {
    #[fk(one2opt)]
    pub id: BlockPointer,
    #[column(index)]
    pub hash: BlockHash
}

#[entity]
pub struct Asset {
    #[fk(one2many, db_cache = 1)]
    pub id: UtxoPointer,
    #[column]
    pub amount: u64,
    #[column(dictionary)]
    pub name: AssetName,
}
