## Ergo Explorer

Ergo explorer on top of [redbit](../../redbit) and [chain](../../chain)

### Installation (Debian/Ubuntu)

```
sudo apt-get install rustup
```

### Usage

```
# Ergo node is expected to run locally at port 9053
export ERGO__API_KEY="foo"           # not needed for syncing
cargo run --release 
```

Indexing might crash especially on laptops with Node running locally and not being synced yet.
In that case, set `fetching_parallelism = "low"` to not put the Node and Laptop under heavy pressure.

### Rest API

http://localhost:3033/swagger-ui/

Querying currently times out during historical indexing. So use it only at the chain tip sync phase
or when indexing is disabled `indexer.enable = false` and we only run http server to query over existing data.

### UI

See [redbit-ui](https://github.com/pragmaxim-com/redbit-ui) 
