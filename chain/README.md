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
    #[write_from(input_refs)]
    pub inputs: Vec<Input>,
    #[column(transient)]
    pub input_refs: Vec<InputRef>,
```

Which expects a custom defined `hook.rs` which lets user turn input refs into redbit standard inputs : 
```rust 
pub(crate) fn write_from_input_refs(tx_context: &TransactionWriteTxContext, parent: BlockPointer, input_index: usize, input_ref: &InputRef) -> Result<Input, AppError> {
    let input =
        match tx_context.transaction_hash_index.get(input_ref.tx_hash)?.next() {
            Some(Ok(tx_pointer)) => {
                let id = TransactionPointer::from_parent(parent, input_index as u16);
                let utxo_pointer = TransactionPointer::from_parent(tx_pointer.value(), input_ref.index as u16);
                Input { id, utxo_pointer }
            },
            _ => {
                let id = TransactionPointer::from_parent(parent, input_index as u16);
                let utxo_pointer = TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0); // genesis of unknown index
                Input { id, utxo_pointer }
            },
        };
    Ok(input)
}
```

- [Demo](../chains/btc)
- [Btc](../chains/btc)
- [Cardano](../chains/cardano)
- [Ergo](../chains/ergo)

### ⏱️ Syncing performance Summary

If throughput does not reach your expectations, check that `buffer` is high enough. This is log from indexing from remote bitcoin node on very poor connection :
```
[2025-08-11 04:45:57] INFO 3 Blocks @ 566011 at 18621.8 ins+outs+asset/s, total 1870059548, buffer: 0
```

1. If it is close to `0`, it means your block fetching or processing is too slow and persistence tasks are idling.
2. If Indexing is under 50 000 inputs+outputs/s with full `buffer`, it means you need more RAM or better SSD.

Hand-made criterion benchmarks [deployed](https://pragmaxim-com.github.io/redbit/report/index.html).

Indexing speed in logs is the **average**, for example, the first ~ 50k **bitcoin** blocks with few Txs have lower in+out/s indexing throughput
and higher blocks/s throughput.

My throughput results after indexing whole bitcoin :

- `2.0GHz` & `NVMe PCIe Gen3` & `DDR4 2933MHz 2Rx4` : `~ 60 000 Inputs+outputs / s`
- `3.0GHz` & `NVMe PCIe Gen4` & `DDR4 3200MHz 4Rx4` : `~ 120 000 Inputs+outputs / s`
- `3.5GHz` & `NVMe PCIe Gen5` & `DDR5 4800MHz 4RX8` : `~ 240 000 Inputs+outputs / s`

In a nutshell, whole bitcoin up to height ~ 0.9M can be indexed in one afternoon with enough RAM for the Linux VM (page cache).