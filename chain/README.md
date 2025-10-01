## Chain

Chain keeps you in sync with arbitrary blockchain if you implement the [api](src/api.rs).

Chain tip is "eventually consistent" with the settlement layer through eager fork competition such that 
superseded forks are immediately deleted from DB and replaced with more valuable fork when it appears.
Ie. only one winning fork is kept in the DB at given moment. This allows for much better performance and space efficiency.

Utxo state is built on the fly during indexing, addresses are stored as a dictionary for deduplication purposes.

### Perf 

Chain syncing uses 3 main independent threads : block fetching, processing and persistence. 
Persistence thread persists block after block sequentially, submitting batches of values to either : 
 - one child thread spawned for each entity column
 - multiple child threads spawned for each shard if column is sharded

Without sharding, indexing would be only as fast as the slowest column indexing due to higher quantity or size.
If you identify a bottleneck column, just shard it so it is as fast as the others. Try to shard columns by DB sizes :
```
  #[column(index, shards = 2)]
  pub hash: TxHash,
  #[column(dictionary, shards = 4)]
  pub address: Address,
```
Here at height `362 544` of bitcoin, we have 2 shards for `transaction_hash_index` and 4 shards for `utxo_address_dict` and they have similar sizes :
```
5.7G    /opt/.chain/main/btc/input_utxo_pointer_by_id.db
5.0G    /opt/.chain/main/btc/transaction_hash_index-0.db
5.0G    /opt/.chain/main/btc/transaction_hash_index-1.db
5.2G    /opt/.chain/main/btc/utxo_address_dict-0.db
5.1G    /opt/.chain/main/btc/utxo_address_dict-1.db
5.2G    /opt/.chain/main/btc/utxo_address_dict-2.db
5.0G    /opt/.chain/main/btc/utxo_address_dict-3.db
```

Due to this concurrency model, if you keep adding new columns, indexing speed will not be affected much until you fully utilize SSD.
It is also a unique and first of its kind way to keep utxo state valid at any time regardless of crashes while reaching the maximum
indexing throughput and CPU utilization possible. As we parallelize indexing while indexing block by block and transaction by transaction.

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

And then all you need to do is defining entities with minimal required fields about resolving transaction inputs : 
```
    #[write_from(input_refs)]
    pub inputs: Vec<Input>,
    #[column(transient)]
    pub input_refs: Vec<InputRef>,
```
With a custom defined [hook.rs](../chains/demo/src/hook.rs) which lets user turn input refs into redbit standard inputs.

And [block_provider.rs](../chains/demo/src/block_provider.rs) that fetches blocks from utxo-like blockchain of your choice.

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
2. If indexing is under 50 000 inputs+outputs/s with full `buffer`, it means you need more RAM or better SSD.

Hand-made criterion benchmarks [deployed](https://pragmaxim-com.github.io/redbit/report/index.html).

Indexing speed in logs is the **average**, for example, the first ~ 50k **bitcoin** blocks with few Txs have lower in+out/s indexing throughput
and higher blocks/s throughput.

My throughput results after indexing whole bitcoin :

- `2.0GHz` & `NVMe PCIe Gen3` & `DDR3 2100MHz 2Rx4` : `~ 70 000 Inputs+outputs / s`
- `3.0GHz` & `NVMe PCIe Gen4` & `DDR4 3200MHz 4Rx4` : `~ 110 000 Inputs+outputs / s`
- `3.5GHz` & `NVMe PCIe Gen5` & `DDR5 4800MHz 4RX8` : `~ 170 000 Inputs+outputs / s`


The size of databases corresponds to bitcoin databases, note that I index both `address` and `script_hash` :
```
$ du -sh * | sort -rh
1.3T    .
427G	utxo_address_dict.db
370G	utxo_script_hash_dict.db
170G	transaction_hash_index.db
105G	utxo_amount_by_id.db
95G	    input_utxo_pointer_by_id.db
53G	    utxo_id.db
48G	    input_id.db
15G	    transaction_id.db
122M	header_prev_hash_index.db
122M	header_merkle_root_index.db
122M	header_hash_index.db
45M	    header_timestamp_index.db
7.2M	header_height.db
7.2M	block_height.db
```

In a nutshell, whole bitcoin up to height ~ 0.9M can be indexed in half a day with enough RAM for the Linux VM (page cache).