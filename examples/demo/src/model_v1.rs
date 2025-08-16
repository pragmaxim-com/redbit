pub use redbit::*;
use syncer::api::{BlockHeaderLike, BlockLike};

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
#[column]
#[derive(Copy, Hash)]
pub struct Timestamp(pub u32);

#[column]
pub struct TempInputRef {
    pub tx_hash: TxHash,
    pub index: u32,
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
    pub hash: BlockHash,
    #[column(index)]
    pub prev_hash: BlockHash,
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
    pub hash: TxHash,
    pub utxos: Vec<Utxo>,
    pub inputs: Vec<InputRef>,
    pub maybe_value: Option<MaybeValue>, // just to demonstrate option is possible
    #[column(transient)]
    pub transient_inputs: Vec<TempInputRef>,
}

#[entity]
pub struct Utxo {
    #[fk(one2many)]
    pub id: TransactionPointer,
    #[column]
    pub amount: u64,
    #[column(dictionary(cache = 100000))]
    pub address: Address,
    pub assets: Vec<Asset>,
}

#[entity]
pub struct InputRef {
    #[fk(one2many)]
    pub id: TransactionPointer,
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
    #[fk(one2many)]
    pub id: UtxoPointer,
    #[column]
    pub amount: u64,
    #[column(dictionary(cache = 100000))]
    pub name: AssetName,
}

impl BlockHeaderLike for BlockHeader {
    fn height(&self) -> u32 {
        self.height.0
    }
    fn hash(&self) -> [u8; 32] {
        self.hash.0
    }
    fn prev_hash(&self) -> [u8; 32] {
        self.prev_hash.0
    }
    fn timestamp(&self) -> u32 {
        self.timestamp.0
    }
}

impl BlockLike for Block {
    type Header = BlockHeader;
    fn header(&self) -> &Self::Header {
        &self.header
    }
    fn weight(&self) -> u32 {
        self.weight
    }
}
