pub use redbit::*;
pub use chain::*;
use crate::block_chain::BlockChain;

#[root_key] pub struct Height(pub u32);

#[pointer_key(u16)] pub struct BlockPointer(Height);
#[pointer_key(u16)] pub struct TransactionPointer(BlockPointer);
#[pointer_key(u8)] pub struct UtxoPointer(TransactionPointer);

#[column] pub struct AssetAction(pub u8);
#[column("utf-8")] pub struct AssetName(pub Vec<u8>);
#[column("hex")] pub struct Tree(pub Vec<u8>);
#[column("hex")] pub struct TreeTemplate(pub Vec<u8>);
#[column("hex")] pub struct BoxId(pub Vec<u8>);

#[column("hex")] pub struct BlockHash(pub [u8; 32]);
#[column("hex")] pub struct TxHash(pub [u8; 32]);
#[column] pub struct Weight(pub u32);
#[column] pub struct BlockTimestamp(pub u32);
#[column("crate::codec::Base58")] pub struct Address(pub Vec<u8>);

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
    pub timestamp: BlockTimestamp,
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
    pub box_ids: Vec<BoxId>,
}

#[entity]
pub struct Utxo {
    #[fk(one2many)]
    pub id: TransactionPointer,
    #[column]
    pub amount: u64,
    #[column(index)]
    pub box_id: BoxId,
    #[column(dictionary)]
    pub address: Address,
    #[column(dictionary)]
    pub tree: Tree,
    #[column(dictionary)]
    pub tree_template: TreeTemplate,
    pub assets: Vec<Asset>,
}

#[entity]
pub struct Asset {
    #[fk(one2many, range)]
    pub id: UtxoPointer,
    #[column]
    pub amount: u64,
    #[column(dictionary)]
    pub name: AssetName,
    #[column(index)]
    pub asset_action: AssetAction,
}

#[entity]
pub struct Input {
    #[fk(one2many)]
    pub id: TransactionPointer,
}
