use syncer::api::{BlockHeaderLike, BlockLike, ChainSyncError};
use chrono::DateTime;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallas::network::miniprotocols::{blockfetch, chainsync, localstate};
pub use redbit::*;
use std::fmt;

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
#[pointer_key(u16)] pub struct UtxoPointer(TransactionPointer);

#[column] pub struct Slot(pub u32);
#[column("hex")] pub struct BlockHash(pub [u8; 32]);
#[column("hex")] pub struct TxHash(pub [u8; 32]);
#[column("hex")] pub struct ScriptHash(pub Vec<u8>);
#[column("hex")] pub struct PolicyId(pub [u8; 28]);
#[column("utf-8")] pub struct AssetName(pub Vec<u8>);
#[column("crate::codec::BaseOrBech")] pub struct Address(pub Vec<u8>);

#[column] pub struct AssetAction(pub u8);

#[column]
pub struct TempInputRef {
    pub tx_hash: TxHash,
    pub index: u32,
}

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
    pub id: Height,
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    #[column(transient)]
    pub weight: u32,
}

#[entity]
pub struct BlockHeader {
    #[fk(one2one)]
    pub id: Height,
    #[column(index)]
    pub hash: BlockHash,
    #[column(index)]
    pub prev_hash: BlockHash,
    #[column(range)]
    pub slot: Slot,
    #[column(range)]
    pub timestamp: BlockTimestamp,
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
    pub transient_inputs: Vec<TempInputRef>,
}

#[entity]
pub struct Utxo {
    #[fk(one2many)]
    pub id: TransactionPointer,
    #[column]
    pub amount: u64,
    #[column(dictionary)]
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
    #[column(dictionary)]
    pub name: AssetName,
    #[column(dictionary)]
    pub policy_id: PolicyId,
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
        self.id.0
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

#[derive(Debug, thiserror::Error)]
pub enum ExplorerError {
    #[error("Cardano chain sync error: {0}")]
    ChainSyncError(#[from] chainsync::ClientError),

    #[error("Cardano block fetch error: {0}")]
    BlockFetchError(#[from] blockfetch::ClientError),

    #[error("Cardano local state error: {0}")]
    LocalStateError(#[from] localstate::ClientError),

    #[error("Cardano pallas traverse error: {0}")]
    PallasTraverseError(#[from] pallas_traverse::Error),

    #[error("Custom error: {0}")]
    Custom(String),
}

impl From<ExplorerError> for ChainSyncError {
    fn from(err: ExplorerError) -> Self {
        ChainSyncError::new(&err.to_string())
    }
}
