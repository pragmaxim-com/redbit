pub use redbit::*;

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
pub struct TempInputRef {
    pub tx_hash: TxHash,
    pub index: u32,
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
    #[column(dictionary(cache = 10000))]
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
    #[column(dictionary(cache = 10000))]
    pub name: AssetName,
}

use syncer::api::*;

pub struct BlockChain {
    pub storage: Arc<Storage>,
}

impl BlockChain {
    pub fn new(storage: Arc<Storage>) -> Arc<dyn BlockChainLike<Block>> {
        Arc::new(BlockChain { storage })
    }

    fn resolve_tx_inputs(&self, read_tx: &StorageReadTx, block: &mut Block) -> Result<(), ChainSyncError> {
        for tx in &mut block.transactions {
            for transient_input in tx.transient_inputs.iter_mut() {
                let tx_pointers = Transaction::get_ids_by_hash(read_tx, &transient_input.tx_hash)?;

                match tx_pointers.first() {
                    Some(tx_pointer) => tx.inputs.push(InputRef { id: TransactionPointer::from_parent(*tx_pointer, transient_input.index as u16) }),
                    None => tx.inputs.push(InputRef { id: TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0) }),
                }
            }
        }
        Ok(())
    }
}
