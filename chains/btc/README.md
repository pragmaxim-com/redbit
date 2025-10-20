## Bitcoin Explorer

Bitcoin explorer on top of [redbit](../../redbit) and [chain](../../chain)

### Installation (Debian/Ubuntu)

```
sudo apt-get install rustup
```

### Requirements

With current [setup](src/model_v1.rs) you need at least 16 cores CPU and 32GB RAM to run both BTC Node and Redbit efficiently, 
because 3 columns are sharded into 15 shards and those 15 threads are busy by sorting and writing big batches.

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

### Bitcoin table export at height 916 467

```json
{
  "height": {
    "table_name": "block_height",
    "table_entries": 916467,
    "tree_height": 3,
    "leaf_pages": 1789,
    "branch_pages": 25,
    "stored_leaf_bytes": 3665868,
    "metadata_bytes": 58020,
    "fragmented_bytes": 3706256
  },
  "header": {
    "height": {
      "table_name": "header_height",
      "table_entries": 916467,
      "tree_height": 3,
      "leaf_pages": 1789,
      "branch_pages": 25,
      "stored_leaf_bytes": 3665868,
      "metadata_bytes": 58020,
      "fragmented_bytes": 3706256
    },
    "hash": [
      {
        "table_name": "pk_by_index",
        "table_entries": 916467,
        "tree_height": 4,
        "leaf_pages": 14624,
        "branch_pages": 265,
        "stored_leaf_bytes": 3665868,
        "metadata_bytes": 38461011,
        "fragmented_bytes": 18858465
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 916467,
        "tree_height": 4,
        "leaf_pages": 16078,
        "branch_pages": 220,
        "stored_leaf_bytes": 32992812,
        "metadata_bytes": 521508,
        "fragmented_bytes": 33242288
      }
    ],
    "prev_hash": [
      {
        "table_name": "pk_by_index",
        "table_entries": 916467,
        "tree_height": 4,
        "leaf_pages": 14593,
        "branch_pages": 265,
        "stored_leaf_bytes": 3665868,
        "metadata_bytes": 38459151,
        "fragmented_bytes": 18733349
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 916467,
        "tree_height": 4,
        "leaf_pages": 16078,
        "branch_pages": 220,
        "stored_leaf_bytes": 32992812,
        "metadata_bytes": 521508,
        "fragmented_bytes": 33242288
      }
    ],
    "timestamp": [
      {
        "table_name": "pk_by_index",
        "table_entries": 916467,
        "tree_height": 3,
        "leaf_pages": 7571,
        "branch_pages": 103,
        "stored_leaf_bytes": 3665868,
        "metadata_bytes": 12154892,
        "fragmented_bytes": 15611944
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 916467,
        "tree_height": 3,
        "leaf_pages": 3579,
        "branch_pages": 49,
        "stored_leaf_bytes": 7331736,
        "metadata_bytes": 116068,
        "fragmented_bytes": 7412484
      }
    ],
    "merkle_root": [
      {
        "table_name": "pk_by_index",
        "table_entries": 916467,
        "tree_height": 4,
        "leaf_pages": 14605,
        "branch_pages": 262,
        "stored_leaf_bytes": 3665868,
        "metadata_bytes": 38459775,
        "fragmented_bytes": 18769589
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 916467,
        "tree_height": 4,
        "leaf_pages": 16078,
        "branch_pages": 220,
        "stored_leaf_bytes": 32992812,
        "metadata_bytes": 521508,
        "fragmented_bytes": 33242288
      }
    ]
  },
  "transactions": {
    "id": {
      "table_name": "transaction_id",
      "table_entries": 1247925363,
      "tree_height": 5,
      "leaf_pages": 3648904,
      "branch_pages": 53660,
      "stored_leaf_bytes": 7487552178,
      "metadata_bytes": 125779826,
      "fragmented_bytes": 7552370140
    },
    "hash": [
      {
        "table_name": "pk_by_index",
        "table_entries": 1247925363,
        "tree_height": 6,
        "leaf_pages": 20627089,
        "branch_pages": 433177,
        "stored_leaf_bytes": 7487552178,
        "metadata_bytes": 52416426749,
        "fragmented_bytes": 26358870609
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 1247925363,
        "tree_height": 5,
        "leaf_pages": 23109728,
        "branch_pages": 339847,
        "stored_leaf_bytes": 47421163794,
        "metadata_bytes": 796605826,
        "fragmented_bytes": 47831689580
      }
    ],
    "utxos": {
      "id": {
        "table_name": "utxo_id",
        "table_entries": 3456967034,
        "tree_height": 5,
        "leaf_pages": 13503777,
        "branch_pages": 210996,
        "stored_leaf_bytes": 27655736272,
        "metadata_bytes": 492887812,
        "fragmented_bytes": 28027086124
      },
      "amount": {
        "table_name": "utxo_amount_by_id",
        "table_entries": 3456967034,
        "tree_height": 5,
        "leaf_pages": 27007554,
        "branch_pages": 421991,
        "stored_leaf_bytes": 55311472544,
        "metadata_bytes": 985775624,
        "fragmented_bytes": 56054168152
      },
      "script_hash": [
        {
          "table_name": "dict_pk_to_ids",
          "table_entries": 3456967034,
          "tree_height": 9,
          "leaf_pages": 22017583,
          "branch_pages": 1321326,
          "stored_leaf_bytes": 27655736272,
          "metadata_bytes": 25805713771,
          "fragmented_bytes": 42134721221
        },
        {
          "table_name": "value_by_dict_pk",
          "table_entries": 1468877467,
          "tree_height": 5,
          "leaf_pages": 26241052,
          "branch_pages": 410014,
          "stored_leaf_bytes": 48544626935,
          "metadata_bytes": 6833308156,
          "fragmented_bytes": 53787923725
        },
        {
          "table_name": "value_to_dict_pk",
          "table_entries": 1468877467,
          "tree_height": 8,
          "leaf_pages": 19272039,
          "branch_pages": 388410,
          "stored_leaf_bytes": 48544626935,
          "metadata_bytes": 7016358874,
          "fragmented_bytes": 24974492463
        },
        {
          "table_name": "dict_pk_by_id",
          "table_entries": 3456967034,
          "tree_height": 5,
          "leaf_pages": 27007554,
          "branch_pages": 421991,
          "stored_leaf_bytes": 55311472544,
          "metadata_bytes": 985775624,
          "fragmented_bytes": 56054168152
        }
      ],
      "address": [
        {
          "table_name": "dict_pk_to_ids",
          "table_entries": 3456967034,
          "tree_height": 10,
          "leaf_pages": 21851817,
          "branch_pages": 1303483,
          "stored_leaf_bytes": 27655736272,
          "metadata_bytes": 25337517338,
          "fragmented_bytes": 41850855190
        },
        {
          "table_name": "value_by_dict_pk",
          "table_entries": 1441721206,
          "tree_height": 5,
          "leaf_pages": 23780747,
          "branch_pages": 371572,
          "stored_leaf_bytes": 43514084845,
          "metadata_bytes": 6634881988,
          "fragmented_bytes": 48778931791
        },
        {
          "table_name": "value_to_dict_pk",
          "table_entries": 1441721206,
          "tree_height": 6,
          "leaf_pages": 17371937,
          "branch_pages": 328067,
          "stored_leaf_bytes": 43514084845,
          "metadata_bytes": 6723894989,
          "fragmented_bytes": 22261236550
        },
        {
          "table_name": "dict_pk_by_id",
          "table_entries": 3456967034,
          "tree_height": 5,
          "leaf_pages": 27007554,
          "branch_pages": 421991,
          "stored_leaf_bytes": 55311472544,
          "metadata_bytes": 985775624,
          "fragmented_bytes": 56054168152
        }
      ]
    },
    "inputs": {
      "id": {
        "table_name": "input_id",
        "table_entries": 3126654331,
        "tree_height": 5,
        "leaf_pages": 12213493,
        "branch_pages": 190834,
        "stored_leaf_bytes": 25013234648,
        "metadata_bytes": 445792404,
        "fragmented_bytes": 25349096340
      },
      "utxo_pointer": {
        "table_name": "input_utxo_pointer_by_id",
        "table_entries": 3126654331,
        "tree_height": 5,
        "leaf_pages": 24426986,
        "branch_pages": 381669,
        "stored_leaf_bytes": 50026469296,
        "metadata_bytes": 891584872,
        "fragmented_bytes": 50698196712
      }
    }
  }
}
```