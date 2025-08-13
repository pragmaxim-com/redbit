## Chain Syncer

Chain syncer keeps you in sync with arbitrary blockchain if you implement the [api](src/api.rs).

Chain tip is "eventually consistent" with the settlement layer through eager fork competition such that 
superseded forks are immediately deleted from DB and replaced with more valuable fork when it appears.
Ie. only one winning fork is kept in the DB at given moment. This allows for much better performance and space efficiency.

### Perf 

Chain syncer uses 3 main independent threads : block fetching, processing and persistence while persistence being sequential, block after block.

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
chain-syncer = { git = "https://github.com/pragmaxim-com/chain-syncer" }
```

- [Bitcoin Explorer](https://github.com/pragmaxim-com/bitcoin-explorer)
- [Cardano Explorer](https://github.com/pragmaxim-com/cardano-explorer)
- [Ergo Explorer](https://github.com/pragmaxim-com/ergo-explorer)