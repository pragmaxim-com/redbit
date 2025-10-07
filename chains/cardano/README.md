## Cardano Explorer

Cardano explorer on top of [redbit](../../redbit) and [chain](../../chain)

### Installation (Debian/Ubuntu)

```
sudo apt-get install rustup
```

### Requirements

With current [setup](src/model_v1.rs) you need at least 16 cores CPU and 32GB RAM to run both Cardano Node and Redbit without crashing.

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

### Cardano table export at height 12 447 014

```json
{
  "height": {
    "table_name": "block_height",
    "table_entries": 12447014,
    "tree_height": 4,
    "leaf_pages": 24310,
    "branch_pages": 333,
    "stored_leaf_bytes": 49788056,
    "metadata_bytes": 788548,
    "fragmented_bytes": 50361124
  },
  "header": {
    "height": {
      "table_name": "blockheader_height",
      "table_entries": 12447014,
      "tree_height": 4,
      "leaf_pages": 24310,
      "branch_pages": 333,
      "stored_leaf_bytes": 49788056,
      "metadata_bytes": 788548,
      "fragmented_bytes": 50361124
    },
    "hash": [
      {
        "table_name": "pk_by_index",
        "table_entries": 12447014,
        "tree_height": 5,
        "leaf_pages": 198862,
        "branch_pages": 4158,
        "stored_leaf_bytes": 49788056,
        "metadata_bytes": 522392294,
        "fragmented_bytes": 259389570
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 12447014,
        "tree_height": 4,
        "leaf_pages": 218368,
        "branch_pages": 2990,
        "stored_leaf_bytes": 448092504,
        "metadata_bytes": 7083428,
        "fragmented_bytes": 451506436
      }
    ],
    "prev_hash": [
      {
        "table_name": "pk_by_index",
        "table_entries": 12447014,
        "tree_height": 5,
        "leaf_pages": 198860,
        "branch_pages": 4161,
        "stored_leaf_bytes": 49788056,
        "metadata_bytes": 522392270,
        "fragmented_bytes": 259393690
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 12447014,
        "tree_height": 4,
        "leaf_pages": 218368,
        "branch_pages": 2990,
        "stored_leaf_bytes": 448092504,
        "metadata_bytes": 7083428,
        "fragmented_bytes": 451506436
      }
    ],
    "slot": [
      {
        "table_name": "pk_by_index",
        "table_entries": 12447014,
        "tree_height": 4,
        "leaf_pages": 102867,
        "branch_pages": 1409,
        "stored_leaf_bytes": 49788056,
        "metadata_bytes": 165147986,
        "fragmented_bytes": 212178454
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 12447014,
        "tree_height": 4,
        "leaf_pages": 48621,
        "branch_pages": 666,
        "stored_leaf_bytes": 99576112,
        "metadata_bytes": 1577156,
        "fragmented_bytes": 100726284
      }
    ],
    "timestamp": [
      {
        "table_name": "pk_by_index",
        "table_entries": 12447014,
        "tree_height": 4,
        "leaf_pages": 102867,
        "branch_pages": 1409,
        "stored_leaf_bytes": 49788056,
        "metadata_bytes": 165147986,
        "fragmented_bytes": 212178454
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 12447014,
        "tree_height": 4,
        "leaf_pages": 48621,
        "branch_pages": 666,
        "stored_leaf_bytes": 99576112,
        "metadata_bytes": 1577156,
        "fragmented_bytes": 100726284
      }
    ]
  },
  "transactions": {
    "id": {
      "table_name": "transaction_id",
      "table_entries": 114216552,
      "tree_height": 4,
      "leaf_pages": 333966,
      "branch_pages": 4911,
      "stored_leaf_bytes": 685299312,
      "metadata_bytes": 11511966,
      "fragmented_bytes": 691228914
    },
    "hash": [
      {
        "table_name": "pk_by_index",
        "table_entries": 114216552,
        "tree_height": 5,
        "leaf_pages": 1888484,
        "branch_pages": 34713,
        "stored_leaf_bytes": 685299312,
        "metadata_bytes": 4797298432,
        "fragmented_bytes": 2394817168
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 114216552,
        "tree_height": 5,
        "leaf_pages": 2115121,
        "branch_pages": 31104,
        "stored_leaf_bytes": 4340228976,
        "metadata_bytes": 72909412,
        "fragmented_bytes": 4377799212
      }
    ],
    "utxos": {
      "id": {
        "table_name": "utxo_id",
        "table_entries": 329593501,
        "tree_height": 5,
        "leaf_pages": 1287474,
        "branch_pages": 20116,
        "stored_leaf_bytes": 2636748008,
        "metadata_bytes": 46992744,
        "fragmented_bytes": 2672147888
      },
      "amount": {
        "table_name": "utxo_amount_by_id",
        "table_entries": 329593501,
        "tree_height": 5,
        "leaf_pages": 2574949,
        "branch_pages": 40233,
        "stored_leaf_bytes": 5273496016,
        "metadata_bytes": 93985588,
        "fragmented_bytes": 5344303868
      },
      "address": [
        {
          "table_name": "dict_pk_to_ids",
          "table_entries": 329593501,
          "tree_height": 9,
          "leaf_pages": 1499312,
          "branch_pages": 148528,
          "stored_leaf_bytes": 2636748008,
          "metadata_bytes": 946584721,
          "fragmented_bytes": 3166219911
        },
        {
          "table_name": "value_by_dict_pk",
          "table_entries": 52227405,
          "tree_height": 5,
          "leaf_pages": 1712907,
          "branch_pages": 26764,
          "stored_leaf_bytes": 3386879331,
          "metadata_bytes": 271430688,
          "fragmented_bytes": 3467615869
        },
        {
          "table_name": "value_to_dict_pk",
          "table_entries": 52227405,
          "tree_height": 5,
          "leaf_pages": 1269885,
          "branch_pages": 38500,
          "stored_leaf_bytes": 3386879331,
          "metadata_bytes": 326065345,
          "fragmented_bytes": 1646646748
        },
        {
          "table_name": "dict_pk_by_id",
          "table_entries": 329593501,
          "tree_height": 5,
          "leaf_pages": 2574949,
          "branch_pages": 40233,
          "stored_leaf_bytes": 5273496016,
          "metadata_bytes": 93985588,
          "fragmented_bytes": 5344303868
        }
      ],
      "script_hash": [
        {
          "table_name": "dict_pk_to_ids",
          "table_entries": 329593501,
          "tree_height": 7,
          "leaf_pages": 1287546,
          "branch_pages": 20119,
          "stored_leaf_bytes": 2636748008,
          "metadata_bytes": 47154076,
          "fragmented_bytes": 2672293756
        },
        {
          "table_name": "value_by_dict_pk",
          "table_entries": 9332,
          "tree_height": 4,
          "leaf_pages": 8451,
          "branch_pages": 133,
          "stored_leaf_bytes": 54323209,
          "metadata_bytes": 345788,
          "fragmented_bytes": 18121019
        },
        {
          "table_name": "value_to_dict_pk",
          "table_entries": 9332,
          "tree_height": 12,
          "leaf_pages": 8333,
          "branch_pages": 6178,
          "stored_leaf_bytes": 54323209,
          "metadata_bytes": 53757105,
          "fragmented_bytes": 34276166
        },
        {
          "table_name": "dict_pk_by_id",
          "table_entries": 329593501,
          "tree_height": 5,
          "leaf_pages": 2574949,
          "branch_pages": 40233,
          "stored_leaf_bytes": 5273496016,
          "metadata_bytes": 93985588,
          "fragmented_bytes": 5344303868
        }
      ],
      "assets": {
        "id": {
          "table_name": "asset_id",
          "table_entries": 1094140882,
          "tree_height": 5,
          "leaf_pages": 5337272,
          "branch_pages": 88954,
          "stored_leaf_bytes": 10941408820,
          "metadata_bytes": 205662830,
          "fragmented_bytes": 11078750046
        },
        "amount": {
          "table_name": "asset_amount_by_id",
          "table_entries": 1094140882,
          "tree_height": 5,
          "leaf_pages": 9597727,
          "branch_pages": 159961,
          "stored_leaf_bytes": 19694535876,
          "metadata_bytes": 369832344,
          "fragmented_bytes": 19903121828
        },
        "name": [
          {
            "table_name": "dict_pk_to_ids",
            "table_entries": 1094140882,
            "tree_height": 9,
            "leaf_pages": 4373409,
            "branch_pages": 1045306,
            "stored_leaf_bytes": 10941408820,
            "metadata_bytes": 399710507,
            "fragmented_bytes": 10853937313
          },
          {
            "table_name": "value_by_dict_pk",
            "table_entries": 10560361,
            "tree_height": 4,
            "leaf_pages": 150622,
            "branch_pages": 2510,
            "stored_leaf_bytes": 269302962,
            "metadata_bytes": 48045366,
            "fragmented_bytes": 309880344
          },
          {
            "table_name": "value_to_dict_pk",
            "table_entries": 10560361,
            "tree_height": 4,
            "leaf_pages": 118251,
            "branch_pages": 2053,
            "stored_leaf_bytes": 269302962,
            "metadata_bytes": 48067734,
            "fragmented_bytes": 175394488
          },
          {
            "table_name": "dict_pk_by_id",
            "table_entries": 1094140882,
            "tree_height": 5,
            "leaf_pages": 10622727,
            "branch_pages": 177044,
            "stored_leaf_bytes": 21882817640,
            "metadata_bytes": 409329000,
            "fragmented_bytes": 21943715376
          }
        ],
        "policy_id": [
          {
            "table_name": "dict_pk_to_ids",
            "table_entries": 1094140882,
            "tree_height": 8,
            "leaf_pages": 5306829,
            "branch_pages": 126963,
            "stored_leaf_bytes": 10941408820,
            "metadata_bytes": 209872539,
            "fragmented_bytes": 11105530673
          },
          {
            "table_name": "value_by_dict_pk",
            "table_entries": 221947,
            "tree_height": 3,
            "leaf_pages": 4110,
            "branch_pages": 68,
            "stored_leaf_bytes": 8433986,
            "metadata_bytes": 158322,
            "fragmented_bytes": 8520780
          },
          {
            "table_name": "value_to_dict_pk",
            "table_entries": 221947,
            "tree_height": 3,
            "leaf_pages": 2994,
            "branch_pages": 65,
            "stored_leaf_bytes": 8433986,
            "metadata_bytes": 169692,
            "fragmented_bytes": 3925986
          },
          {
            "table_name": "dict_pk_by_id",
            "table_entries": 1094140882,
            "tree_height": 5,
            "leaf_pages": 10622727,
            "branch_pages": 177044,
            "stored_leaf_bytes": 21882817640,
            "metadata_bytes": 409329000,
            "fragmented_bytes": 21943715376
          }
        ],
        "asset_action": [
          {
            "table_name": "pk_by_index",
            "table_entries": 1094140882,
            "tree_height": 6,
            "leaf_pages": 5337272,
            "branch_pages": 88955,
            "stored_leaf_bytes": 10941408820,
            "metadata_bytes": 205662872,
            "fragmented_bytes": 11078754100
          },
          {
            "table_name": "index_by_pk",
            "table_entries": 1094140882,
            "tree_height": 5,
            "leaf_pages": 5851020,
            "branch_pages": 97516,
            "stored_leaf_bytes": 12035549702,
            "metadata_bytes": 225459238,
            "fragmented_bytes": 12104194516
          }
        ]
      }
    },
    "inputs": {
      "id": {
        "table_name": "input_id",
        "table_entries": 318577646,
        "tree_height": 5,
        "leaf_pages": 1244443,
        "branch_pages": 19444,
        "stored_leaf_bytes": 2548621168,
        "metadata_bytes": 45422124,
        "fragmented_bytes": 2582837860
      },
      "utxo_pointer": {
        "table_name": "input_utxo_pointer_by_id",
        "table_entries": 318577646,
        "tree_height": 5,
        "leaf_pages": 2488887,
        "branch_pages": 38889,
        "stored_leaf_bytes": 5097242336,
        "metadata_bytes": 90844348,
        "fragmented_bytes": 5165683812
      }
    }
  }
}
```