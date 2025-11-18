### Bitcoin Cash

Download Bitcoin Cash Node:
```
wget https://github.com/bitcoin-cash-node/bitcoin-cash-node/releases/download/v28.0.1/bitcoin-cash-node-28.0.1-x86_64-linux-gnu.tar.gz
```

Configure it :
```
$ cat /opt/bitcoincash/bitcoin.conf
server=1
rest=1
rpcbind=127.0.0.1
rpcallowip=127.0.0.1
rpcuser=foo
rpcpassword=bar
disablewallet=1
maxconnections=32
rpcport=7332
port=7333
```

Export RPC credentials :
```
export BITCOINCASH__rpc_user="foo"
export BITCOINCASH__rpc_password="bar"
```

Start the node :
```
/opt/bitcoincash/bin/bitcoind -datadir=/opt/bitcoincash -conf=/opt/bitcoincash/bitcoin.conf -daemon=0 -printtoconsole
```

Start syncing : 
```
cargo run --release
```