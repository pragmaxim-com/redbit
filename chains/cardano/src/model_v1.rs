pub use redbit::*;
pub use chain::*;

#[root_key] pub struct Height(pub u32);

#[pointer_key(u16)] pub struct BlockPointer(Height);
#[pointer_key(u16)] pub struct TransactionPointer(BlockPointer);
#[pointer_key(u16)] pub struct UtxoPointer(TransactionPointer);

#[column] pub struct Slot(pub u32);
#[column("hex")] pub struct BlockHash(pub [u8; 32]);
#[column("hex")] pub struct TxHash(pub [u8; 32]);
#[column("hex")] pub struct ScriptHash(pub Vec<u8>);
#[column("hex")] pub struct PolicyId(pub [u8; 28]);
#[column("utf-8")] pub struct AssetName(pub Vec<u8>);
#[column("crate::codec::BaseOrBech")] pub struct Address(pub Vec<u8>);
#[column] pub struct Weight(pub u32);
#[column] pub struct AssetAction(pub u8);

#[column]
pub struct InputRef {
    pub tx_hash: TxHash,
    pub index: u32,
}

#[column]
pub struct Timestamp(pub u32);

#[entity]
pub struct Block {
    #[pk(db_cache = 1)]
    pub height: Height,
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

#[entity]
pub struct BlockHeader {
    #[fk(one2one, db_cache = 1)]
    pub height: Height,
    #[column(index, db_cache = 1)]
    pub hash: BlockHash,
    #[column(index, db_cache = 1)]
    pub prev_hash: BlockHash,
    #[column(range, db_cache = 1)]
    pub slot: Slot,
    #[column(range, db_cache = 1)]
    pub timestamp: Timestamp,
    #[column(transient)]
    pub weight: Weight,
}

#[entity]
pub struct Transaction {
    #[fk(one2many, db_cache = 2)]
    pub id: BlockPointer,
    #[column(index, used, db_cache = 20, lru_cache = 20)]
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
    #[fk(one2many, db_cache = 1)]
    pub id: TransactionPointer,
    #[column(db_cache = 1)]
    pub amount: u64,
    #[column(dictionary, db_cache = 20, lru_cache = 20)]
    pub script_hash: ScriptHash,
    #[column(dictionary, db_cache = 20, lru_cache = 20)]
    pub address: Address,
    pub assets: Vec<Asset>,
}

#[entity]
pub struct Asset {
    #[fk(one2many, db_cache = 1)]
    pub id: UtxoPointer,
    #[column(db_cache = 1)]
    pub amount: u64,
    #[column(index, db_cache = 2)]
    pub asset_action: AssetAction,
    #[column(dictionary, db_cache = 20, lru_cache = 20)]
    pub name: AssetName,
    #[column(dictionary, db_cache = 20, lru_cache = 20)]
    pub policy_id: PolicyId,
}

#[entity]
pub struct Input {
    #[fk(one2many, db_cache = 1)]
    pub id: TransactionPointer,
    #[column(db_cache = 1)]
    pub utxo_pointer: TransactionPointer,
}
