use std::error::Error;
pub use redbit::*;
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
pub struct InputRef {
    #[fk(one2many)]
    pub id: TransactionPointer,
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

impl From<ExplorerError> for ChainError {
    fn from(err: ExplorerError) -> Self {
        ChainError::new(&err.to_string())
    }
}

use chain::api::*;

pub struct BlockChain {
    pub storage: Arc<Storage>,
}

impl BlockChain {
    pub fn new(storage: Arc<Storage>) -> Arc<dyn BlockChainLike<Block>> {
        Arc::new(BlockChain { storage })
    }

    fn resolve_tx_inputs(&self, read_tx: &StorageReadTx, block: &mut Block) -> Result<(), ChainError> {
        for tx in &mut block.transactions {
            for box_id in tx.transient_inputs.iter_mut() {
                let utxo_pointers = Utxo::get_ids_by_box_id(read_tx, box_id).expect("Failed to get Utxo by ErgoBox");
                match utxo_pointers.first() {
                    Some(utxo_pointer) => {
                        tx.inputs.push(InputRef { id: TransactionPointer::from_parent(utxo_pointer.parent, utxo_pointer.index()) })
                    }
                    None => tx.inputs.push(InputRef { id: TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0) }),
                }
            }
        }
        Ok(())
    }

}
