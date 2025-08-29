## Bitcoin Explorer

Bitcoin explorer on top of [redbit](../../redbit) and [chain](../../chain)

It uses tiny `block_height/tx_index/utxo_index/[asset_index]` dictionary pointers to big hashes, ie. not a single hash is duplicated,
which allows for much better space efficiency and syncing speed with local node and an SSD.

Chain tip is "eventually consistent" through fork competition, ie. forks get settled eventually and superseded forks are deleted from DB.

### Installation (Debian/Ubuntu)

```
sudo apt-get install rustup
```

### Usage

Run bitcoin node locally and enable -rest api which is under port 8332 by default and can be changed in `config/btc.toml`, for example:
```
cat ~/snap/bitcoin-core/common/.bitcoin/bitcoin.conf
rest=1
nodebuglogfile=1
disablewallet=1
blocksonly=1

bitcoin-core.daemon -daemon
```
Then check if node is running and synced:
```
bitcoin-core.cli getblockchaininfo
tail -f ~/snap/bitcoin-core/common/.bitcoin/debug.log
```

Then : 
```
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