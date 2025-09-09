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
```
    #[load_from(input_refs)]
    pub inputs: Vec<Input>,
    #[column(transient)]
    pub input_refs: Vec<InputRef>,
```

Which expects a custom defined `hook.rs` which lets user turn input refs into redbit standard inputs : 
```rust 
    pub(crate) fn load_from_input_refs(tx_context: &TransactionWriteTxContext, transaction: &mut Transaction) -> Result<(), ChainError> {
        for transient_input in transaction.input_refs.iter_mut() {
            match tx_context.transaction_hash_index.get(&transient_input.tx_hash)?.next() {
                Some(Ok(tx_pointer)) => transaction.inputs.push(Input { id: TransactionPointer::from_parent(tx_pointer.value(), transient_input.index as u16) }),
                _ => transaction.inputs.push(Input { id: TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0) }),
            }
        }
        Ok(())
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