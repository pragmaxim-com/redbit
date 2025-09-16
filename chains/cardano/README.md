## Cardano Explorer

Cardano explorer on top of [redbit](../../redbit) and [chain](../../chain)

### Installation (Debian/Ubuntu)

```
sudo apt-get install rustup
```

### Usage

Run cardano node locally, api at port 3001 can be change in `config/cardano.toml`, for example :
```
nohup ./bin/cardano-node run \
    --topology /opt/cardano/share/mainnet/topology.json \
    --config /opt/cardano/share/mainnet/config.json \
    --database-path ~/.cardano/db \
    --socket-path ~/.cardano/db/node.socket \
    --host-addr 0.0.0.0 \
    --port 3001 &
./bin/cardano-cli query tip --mainnet --socket-path ~/.cardano/db/node.socket
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