use bitcoin::block::Bip34Error;
use syncer::api::{BlockHeaderLike, BlockLike, ChainSyncError};
use chrono::DateTime;
pub use redbit::*;
use std::fmt;

#[root_key] pub struct Height(pub u32);

#[pointer_key(u16)] pub struct BlockPointer(Height);
#[pointer_key(u16)] pub struct TransactionPointer(BlockPointer);
#[pointer_key(u8)] pub struct UtxoPointer(TransactionPointer);

#[column] pub struct Hash(pub String);
#[column("hex")] pub struct BlockHash(pub [u8; 32]);
#[column("hex")] pub struct MerkleRoot(pub [u8; 32]);
#[column("hex")] pub struct TxHash(pub [u8; 32]);
#[column("hex")] pub struct ScriptHash(pub Vec<u8>);

#[column("crate::codec::BaseOrBech")]
pub struct Address(pub Vec<u8>);

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
    pub timestamp: BlockTimestamp,
    #[column(index)]
    pub merkle_root: MerkleRoot,
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
    #[column(dictionary(cache = 100000))]
    pub script_hash: ScriptHash,
    #[column(dictionary(cache = 100000))]
    pub address: Address,
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
    #[error("RPC error: {0}")]
    Rpc(#[from] bitcoincore_rpc::Error),

    #[error("Height decoding error: {0}")]
    Bip34(#[from] Bip34Error),
}

impl From<ExplorerError> for ChainSyncError {
    fn from(err: ExplorerError) -> Self {
        ChainSyncError::new(&err.to_string())
    }
}
