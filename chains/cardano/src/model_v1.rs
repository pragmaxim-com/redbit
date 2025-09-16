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
    #[pk]
    pub height: Height,
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

#[entity]
pub struct BlockHeader {
    #[fk(one2one)]
    pub height: Height,
    #[column(index)]
    pub hash: BlockHash,
    #[column(index)]
    pub prev_hash: BlockHash,
    #[column(range)]
    pub slot: Slot,
    #[column(range)]
    pub timestamp: Timestamp,
    #[column(transient)]
    pub weight: Weight,
}

#[entity]
pub struct Transaction {
    #[fk(one2many)]
    pub id: BlockPointer,
    #[column(index(cache = 4))]
    pub hash: TxHash,
    pub utxos: Vec<Utxo>,
    #[write_from(input_refs)]
    pub inputs: Vec<Input>,
    #[column(transient)]
    pub input_refs: Vec<InputRef>,
    #[column(transient(read_from(inputs::utxo_pointer)))]
    pub input_utxos: Vec<Utxo>,
}

#[entity]
pub struct Utxo {
    #[fk(one2many)]
    pub id: TransactionPointer,
    #[column]
    pub amount: u64,
    #[column(dictionary(cache = 10))]
    pub address: Address,
    #[column]
    pub script_hash: ScriptHash,
    pub assets: Vec<Asset>,
}

#[entity]
pub struct Asset {
    #[fk(one2many, range)]
    pub id: UtxoPointer,
    #[column]
    pub amount: u64,
    #[column(dictionary(cache = 2))]
    pub name: AssetName,
    #[column(dictionary(cache = 2))]
    pub policy_id: PolicyId,
    #[column(index)]
    pub asset_action: AssetAction,
}

#[entity]
pub struct Input {
    #[fk(one2many)]
    pub id: TransactionPointer,
    #[column]
    pub utxo_pointer: TransactionPointer,
}
