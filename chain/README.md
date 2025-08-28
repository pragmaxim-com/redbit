## Chain

Chain keeps you in sync with arbitrary blockchain if you implement the [api](src/api.rs).

Chain tip is "eventually consistent" with the settlement layer through eager fork competition such that 
superseded forks are immediately deleted from DB and replaced with more valuable fork when it appears.
Ie. only one winning fork is kept in the DB at given moment. This allows for much better performance and space efficiency.

### Perf 

Chain syncing uses 3 main independent threads : block fetching, processing and persistence while persistence being sequential, block after block.

You can use [tokio console](https://github.com/tokio-rs/console), basically it breaks down to 3 named task you can see in the console :
- fetch - task that fetches blocks from the node
- process - task that transforms the node blocks into redbit blocks
- persist - task that persists redbit blocks into redb

``` 
cargo install --locked tokio-console
RUSTFLAGS="--cfg tokio_unstable" cargo run --features tracing
tokio-console
```

### Usage

```
[features]
default = ["chain"]
chain = ["redbit/chain"]
tracing = ["chain/tracing"]

[dependencies]
redbit = { path = "../../redbit" }
chain = { path = "../../chain" }
```

And then all you need to do is defining entities with minimal required fields and how to resolve transaction inputs : 
```rust

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
    #[column(transient)]
    pub weight: Weight,
}

use chain::api::*;

pub struct BlockChain {
    pub storage: Arc<Storage>,
}

impl BlockChain {
    pub fn new(storage: Arc<Storage>) -> Arc<dyn BlockChainLike<Block>> {
        Arc::new(BlockChain { storage })
    }

    fn resolve_tx_inputs(&self, tx_context: &TransactionReadTxContext, block: &mut Block) -> Result<(), ChainSyncError> {
        for tx in &mut block.transactions {
            for transient_input in tx.transient_inputs.iter_mut() {
                let tx_pointers = Transaction::get_ids_by_hash(tx_context, &transient_input.tx_hash)?;

                match tx_pointers.first() {
                    Some(tx_pointer) => tx.inputs.push(InputRef { id: TransactionPointer::from_parent(*tx_pointer, transient_input.index as u16) }),
                    None => tx.inputs.push(InputRef { id: TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0) }),
                }
            }
        }
        Ok(())
    }
}
```

- [Demo](../chains/btc)
- [Btc](../chains/btc)
- [Cardano](../chains/cardano)
- [Ergo](../chains/ergo)

### Troubleshooting 

If throughput does not reach your expectations, check that `buffer` is high enough : 
```
[2025-08-11 04:45:57] INFO 3 Blocks @ 566011 at 18621.8 ins+outs+asset/s, total 1870059548, buffer: 255
```

if it is close to 0, it means your block fetching or processing is too slow and persistence task is idling.