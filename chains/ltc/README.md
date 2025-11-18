### Litecoin

Note that current version fails on processing blocks since MWEB activation which would require patching the litcoin crate.
It fails right at the start because chain indexer processes the tip block header. 

Download Litecoin Node:
```
wget https://download.litecoin.org/litecoin-0.21.4/linux/litecoin-0.21.4-x86_64-linux-gnu.tar.gz
```

Configure it :
```
$ cat /opt/litecoin/litecoin.conf
server=1
rest=1
rpcbind=127.0.0.1
rpcallowip=127.0.0.1
rpcuser=foo
rpcpassword=bar
disablewallet=1
maxconnections=32
rpcport=9332
port=9333
```

Start the node :
```
/opt/litecoin/bin/litecoind -datadir=/opt/litecoin -conf=/opt/litecoin/litecoin.conf
```

Start syncing (we use only rest-api so no rpc credentials needed):
```
cargo run --release
```