use syncer::api::{BlockHeaderLike, BlockLike, ChainSyncError};
use std::error::Error;
use chrono::DateTime;
pub use redbit::*;
use std::fmt;
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Clone, Copy, Debug, IntoPrimitive, PartialEq, TryFromPrimitive, )]
#[repr(u8)]
pub enum AssetType {
    Mint = 0,
    Transfer = 1,
    Burn = 2,
}

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

#[column("crate::codec::Base58")]
pub struct Address(pub Vec<u8>);

#[column]
#[derive(Copy, Hash)]
pub struct BlockTimestamp(pub u32);
impl fmt::Display for BlockTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let datetime = DateTime::from_timestamp(self.0 as i64, 0).unwrap();
        let readable_date = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
        write!(f, "{}", readable_date)
    }
}

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
    pub weight: u32,
}

#[entity]
pub struct Transaction {
    #[fk(one2many)]
    pub id: BlockPointer,
    #[column(index)]
    pub hash: TxHash,
    pub utxos: Vec<Utxo>,
    pub inputs: Vec<InputRef>,
    #[column(transient)]
    pub transient_inputs: Vec<BoxId>,
}

#[entity]
pub struct Utxo {
    #[fk(one2many)]
    pub id: TransactionPointer,
    #[column]
    pub amount: u64,
    #[column(index)]
    pub box_id: BoxId,
    #[column(dictionary(cache = 10000))]
    pub address: Address,
    #[column(dictionary(cache = 10000))]
    pub tree: Tree,
    #[column(dictionary(cache = 10000))]
    pub tree_template: TreeTemplate,
    pub assets: Vec<Asset>,
}

#[entity]
pub struct Asset {
    #[fk(one2many, range)]
    pub id: UtxoPointer,
    #[column]
    pub amount: u64,
    #[column(dictionary(cache = 10000))]
    pub name: AssetName,
    #[column(index)]
    pub asset_action: AssetAction,
}

#[entity]
pub struct InputRef {
    #[fk(one2many)]
    pub id: TransactionPointer,
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
    fn weight(&self) -> u32 {
        self.weight
    }
}

impl BlockLike for Block {
    type Header = BlockHeader;
    fn header(&self) -> &Self::Header {
        &self.header
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExplorerError {
    #[error("Reqwest error: {source}{}", source.source().map(|e| format!(": {}", e)).unwrap_or_default())]
    Reqwest {
        #[from]
        source: reqwest::Error,
    },

    #[error("Url parsing error: {0}")]
    Url(#[from] url::ParseError),

    #[error("Invalid http header value : {0}")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),

    #[error("Custom error: {0}")]
    Custom(String),
}

impl From<ExplorerError> for ChainSyncError {
    fn from(err: ExplorerError) -> Self {
        ChainSyncError::new(&err.to_string())
    }
}
