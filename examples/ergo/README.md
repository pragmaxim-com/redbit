## Ergo Explorer

Ergo explorer on top of [redbit](https://github.com/pragmaxim-com/redbit) and [chain-syncer](https://github.com/pragmaxim-com/chain-syncer)

It uses tiny `block_height/tx_index/utxo_index/[asset_index]` dictionary pointers to big hashes, ie. not a single hash is duplicated,
which allows for much better space efficiency and syncing speed with local node and an SSD.

Chain tip is "eventually consistent" through fork competition, ie. forks get settled eventually and superseded forks are deleted from DB.

### Installation (Debian/Ubuntu)

```
sudo apt-get install rustup
```

### Usage

```
# Ergo node is expected to run locally at port 9053
export ERGO__API_KEY="foo"
cargo run
```

Indexing might crash especially on laptops with Node running locally and not being synced yet.
In that case, set `fetching_parallelism = "low"` to not put the Node and Laptop under heavy pressure.

### Rest API

http://localhost:8000/swagger-ui/

Querying currently times out during historical indexing. So use it only at the chain tip sync phase
or when indexing is disabled `indexer.enable = false` and we only run http server to query over existing data.

### UI

See [redbit-ui](https://github.com/pragmaxim-com/redbit-ui) 