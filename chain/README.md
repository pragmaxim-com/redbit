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

And then all you need to do is defining entities with minimal required fields about resolving transaction inputs
 - Bitcoin like utxos pointer : 
```
    pub hash: TxHash,
    #[write_from_using(input_refs, hash)]
    pub inputs: Vec<Input>,
    #[column(transient)]
    pub input_refs: Vec<InputRef>,
```
 - Ergo like utxo pointer:
```
    pub utxos: Vec<Utxo>,
    #[write_from_using(input_refs, utxos)]
    pub inputs: Vec<Input>,
    #[column(transient)]
    pub input_refs: Vec<BoxId>,
```

With a custom defined [hook.rs](../chains/demo/src/hook.rs) which lets user turn input refs into redbit standard inputs.
This is because the chain builds utxo-state on the fly and fails fast in case of tx with an IO issue. 

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
2. If indexing is under 150 000 inputs+outputs/s with full `buffer`, it means you need more RAM or better SSD.

Watch indexing tasks reports in logs : 
 - `COLLECT` - Workers are collecting data from the `MASTER` thread 
 - `SORT`    - How much time workers spend for data sorting 
 - `WRITE`   - B+Tree modifications time (insertions) 
 - `FLUSH`   - non-durable commit time 

Here you can see that `transaction_hash_index` is the slowest column, so it can be fixed by sharding it or increase db_cache or lru_cache.

```
TASK (c)ollect,(s)ort,(w)rite,(f)lush ms |     c      s      w      f =  last |     c      s      w      f =   avg |      dev | coefov %
block_height                             |  5186      0      0      0 =  5186 |  5213      0      0      0 =  5213 |     1232 |      221
utxo_address_dict                        |   472      0   3939    773 =  5184 |   426      0   3961    803 =  5190 |     1321 |       55
input_utxo_pointer_by_id                 |  3148    102   1585     18 =  4853 |  3389    144   1418     19 =  4970 |     1129 |      104
input_id                                 |  3149    112   1570      9 =  4840 |  3379    150   1405      9 =  4943 |     1131 |      117
transaction_hash_index                   |  3149      0      0   1560 =  4709 |  3365      0      0   1408 =  4773 |     1059 |       39
utxo_script_hash_dict                    |   411      0   3228   1026 =  4665 |   379      0   3334   1000 =  4713 |     1302 |       60
utxo_id                                  |   386      0   2082     18 =  2486 |   355      0   2162     20 =  2537 |      369 |       70
utxo_amount_by_id                        |   386      0   1858     52 =  2296 |   355      0   1935     43 =  2333 |      363 |       70
transaction_id                           |   386      0    724      5 =  1115 |   355      0    707      4 =  1066 |      167 |      115
header_prev_hash_index                   |   386      0      5      1 =   392 |   355      0      6      1 =   362 |       67 |      224
header_hash_index                        |   386      0      2      3 =   391 |   355      0      6      1 =   362 |       67 |      196
header_merkle_root_index                 |   386      0      2      1 =   389 |   355      0      6      1 =   362 |       66 |      204
header_timestamp_index                   |   386      0      1      1 =   388 |   355      0      0      0 =   355 |       60 |      296
header_height                            |   386      0      0      0 =   386 |   355      0      0      0 =   355 |       60 |     1175
MASTER                                   |   385      0      0      0 =   385 |   355      0      0      0 =   355 |       58 |       16
```

Hand-made criterion benchmarks [deployed](https://pragmaxim-com.github.io/redbit/report/index.html).

Indexing speed in logs is the **average**, for example, the first ~ 50k **bitcoin** blocks with few Txs have lower in+out/s indexing throughput
and higher blocks/s throughput.

With this much RAM :
```
$ ps -p <PID> -o rss,vsz
  RSS    VSZ
38540932 46648556
```

My throughput results after indexing whole bitcoin :

- `2.0GHz` & `NVMe PCIe Gen3` & `DDR3 2100MHz 2Rx4` : `~ 180 000 Inputs+outputs / s`
- `3.0GHz` & `NVMe PCIe Gen4` & `DDR4 3200MHz 4Rx4` : `~ 310 000 Inputs+outputs / s`
- `3.5GHz` & `NVMe PCIe Gen5` & `DDR5 4800MHz 4RX8` : `~ 540 000 Inputs+outputs / s`


The size of databases (w/o sharding) corresponds to bitcoin databases, note that I index both `address` and `script_hash` :
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

In a nutshell, whole bitcoin up to height ~ 0.9M can be indexed in half a day with enough RAM.