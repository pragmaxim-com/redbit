pub use redbit::*;
pub use chain::*;

#[root_key] pub struct Height(pub u32);

#[pointer_key(u16)] pub struct BlockPointer(Height);
#[pointer_key(u16)] pub struct TransactionPointer(BlockPointer);
#[pointer_key(u8)] pub struct UtxoPointer(TransactionPointer);

#[column] pub struct AssetAction(pub u8);
#[column("utf-8")] pub struct AssetName(pub Vec<u8>);
#[column("hex")] pub struct Tree(pub Vec<u8>);
#[column("hex")] pub struct TreeTemplate(pub Vec<u8>);
#[column("hex")] pub struct BoxId(pub [u8; 32]);

#[column("hex")] pub struct BlockHash(pub [u8; 32]);
#[column("hex")] pub struct TxHash(pub [u8; 32]);
#[column] pub struct Weight(pub u32);
#[column] pub struct Timestamp(pub u32);
#[column("crate::codec::Base58")] pub struct Address(pub Vec<u8>);

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
    pub timestamp: Timestamp,
    #[column(transient)]
    pub weight: Weight,
}

#[entity]
pub struct Transaction {
    #[fk(one2many, db_cache = 2)]
    pub id: BlockPointer,
    #[column(index, db_cache = 4)]
    pub hash: TxHash,
    pub utxos: Vec<Utxo>,
    #[write_from_using(input_refs, utxos)] // implement custom write_from_using function, see hook.rs
    pub inputs: Vec<Input>,
    #[column(transient)]
    pub input_refs: Vec<BoxId>,
    #[column(transient(read_from(inputs::utxo_pointer)))]
    pub input_utxos: Vec<Utxo>,
}

#[entity]
pub struct Utxo {
    #[fk(one2many, db_cache = 1)]
    pub id: TransactionPointer,
    #[column(db_cache = 1)]
    pub amount: u64,
    #[column(index, used, db_cache = 30)]
    pub box_id: BoxId,
    #[column(dictionary, lru_cache = 20, db_cache = 30)]
    pub address: Address,
    #[column(dictionary, lru_cache = 20, db_cache = 30)]
    pub tree: Tree,
    #[column(dictionary, lru_cache = 10, db_cache = 20)]
    pub tree_template: TreeTemplate,
    pub assets: Vec<Asset>,
}

#[entity]
pub struct Asset {
    #[fk(one2many, db_cache = 1)]
    pub id: UtxoPointer,
    #[column(db_cache = 1)]
    pub amount: u64,
    #[column(index, lru_cache = 10, db_cache = 10)]
    pub asset_action: AssetAction,
    #[column(dictionary, lru_cache = 10, db_cache = 10)]
    pub name: AssetName,
}

#[entity]
pub struct Input {
    #[fk(one2many, db_cache = 1)]
    pub id: TransactionPointer,
    #[column(db_cache = 1)]
    pub utxo_pointer: TransactionPointer,
}
