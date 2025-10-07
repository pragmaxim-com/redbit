## Ergo Explorer

Ergo explorer on top of [redbit](../../redbit) and [chain](../../chain)

### Installation (Debian/Ubuntu)

```
sudo apt-get install rustup
```

### Requirements

With current [setup](src/model_v1.rs) you need at least 8 cores CPU and 16GB RAM to run both Ergo Node and Redbit without crashing.

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

### Ergo table export at height 1 621 246

```json
{
  "height": {
    "table_name": "block_height",
    "table_entries": 1621246,
    "tree_height": 3,
    "leaf_pages": 3166,
    "branch_pages": 43,
    "stored_leaf_bytes": 6484984,
    "metadata_bytes": 102660,
    "fragmented_bytes": 6556420
  },
  "header": {
    "height": {
      "table_name": "blockheader_height",
      "table_entries": 1621246,
      "tree_height": 3,
      "leaf_pages": 3166,
      "branch_pages": 43,
      "stored_leaf_bytes": 6484984,
      "metadata_bytes": 102660,
      "fragmented_bytes": 6556420
    },
    "hash": [
      {
        "table_name": "pk_by_index",
        "table_entries": 1621246,
        "tree_height": 4,
        "leaf_pages": 25865,
        "branch_pages": 522,
        "stored_leaf_bytes": 6484984,
        "metadata_bytes": 68039634,
        "fragmented_bytes": 33556534
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 1621246,
        "tree_height": 4,
        "leaf_pages": 28442,
        "branch_pages": 390,
        "stored_leaf_bytes": 58364856,
        "metadata_bytes": 922596,
        "fragmented_bytes": 58808420
      }
    ],
    "prev_hash": [
      {
        "table_name": "pk_by_index",
        "table_entries": 1621246,
        "tree_height": 4,
        "leaf_pages": 25976,
        "branch_pages": 522,
        "stored_leaf_bytes": 6484984,
        "metadata_bytes": 68046294,
        "fragmented_bytes": 34004530
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 1621246,
        "tree_height": 4,
        "leaf_pages": 28442,
        "branch_pages": 390,
        "stored_leaf_bytes": 58364856,
        "metadata_bytes": 922596,
        "fragmented_bytes": 58808420
      }
    ],
    "timestamp": [
      {
        "table_name": "pk_by_index",
        "table_entries": 1621246,
        "tree_height": 4,
        "leaf_pages": 13352,
        "branch_pages": 183,
        "stored_leaf_bytes": 6484984,
        "metadata_bytes": 21415963,
        "fragmented_bytes": 27538413
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 1621246,
        "tree_height": 3,
        "leaf_pages": 6332,
        "branch_pages": 86,
        "stored_leaf_bytes": 12969968,
        "metadata_bytes": 205348,
        "fragmented_bytes": 13112812
      }
    ]
  },
  "transactions": {
    "id": {
      "table_name": "transaction_id",
      "table_entries": 9528557,
      "tree_height": 4,
      "leaf_pages": 27861,
      "branch_pages": 409,
      "stored_leaf_bytes": 57171342,
      "metadata_bytes": 960332,
      "fragmented_bytes": 57662246
    },
    "hash": [
      {
        "table_name": "pk_by_index",
        "table_entries": 9528557,
        "tree_height": 4,
        "leaf_pages": 157701,
        "branch_pages": 3368,
        "stored_leaf_bytes": 57171342,
        "metadata_bytes": 400240617,
        "fragmented_bytes": 202326665
      },
      {
        "table_name": "index_by_pk",
        "table_entries": 9528557,
        "tree_height": 4,
        "leaf_pages": 176454,
        "branch_pages": 2595,
        "stored_leaf_bytes": 362085166,
        "metadata_bytes": 6082446,
        "fragmented_bytes": 365217092
      }
    ],
    "utxos": {
      "id": {
        "table_name": "utxo_id",
        "table_entries": 50473558,
        "tree_height": 4,
        "leaf_pages": 197162,
        "branch_pages": 3080,
        "stored_leaf_bytes": 403788464,
        "metadata_bytes": 7196360,
        "fragmented_bytes": 409206408
      },
      "amount": {
        "table_name": "utxo_amount_by_id",
        "table_entries": 50473558,
        "tree_height": 4,
        "leaf_pages": 394324,
        "branch_pages": 6160,
        "stored_leaf_bytes": 807576928,
        "metadata_bytes": 14392752,
        "fragmented_bytes": 818412784
      },
      "box_id": [
        {
          "table_name": "pk_by_index",
          "table_entries": 50473558,
          "tree_height": 5,
          "leaf_pages": 942044,
          "branch_pages": 18274,
          "stored_leaf_bytes": 403788464,
          "metadata_bytes": 2332185634,
          "fragmented_bytes": 1197488430
        },
        {
          "table_name": "index_by_pk",
          "table_entries": 50473558,
          "tree_height": 5,
          "leaf_pages": 1073905,
          "branch_pages": 16779,
          "stored_leaf_bytes": 2018942320,
          "metadata_bytes": 241091708,
          "fragmented_bytes": 2207407636
        }
      ],
      "address": [
        {
          "table_name": "dict_pk_to_ids",
          "table_entries": 50473558,
          "tree_height": 8,
          "leaf_pages": 182055,
          "branch_pages": 26658,
          "stored_leaf_bytes": 403788464,
          "metadata_bytes": 24769215,
          "fragmented_bytes": 426330769
        },
        {
          "table_name": "value_by_dict_pk",
          "table_entries": 1021783,
          "tree_height": 4,
          "leaf_pages": 84295,
          "branch_pages": 1316,
          "stored_leaf_bytes": 196486485,
          "metadata_bytes": 7163832,
          "fragmented_bytes": 147012339
        },
        {
          "table_name": "value_to_dict_pk",
          "table_entries": 1021783,
          "tree_height": 10,
          "leaf_pages": 75071,
          "branch_pages": 14688,
          "stored_leaf_bytes": 196486485,
          "metadata_bytes": 41198920,
          "fragmented_bytes": 130147683
        },
        {
          "table_name": "dict_pk_by_id",
          "table_entries": 50473558,
          "tree_height": 4,
          "leaf_pages": 394324,
          "branch_pages": 6160,
          "stored_leaf_bytes": 807576928,
          "metadata_bytes": 14392752,
          "fragmented_bytes": 818412784
        }
      ],
      "tree": [
        {
          "table_name": "dict_pk_to_ids",
          "table_entries": 50473558,
          "tree_height": 8,
          "leaf_pages": 182055,
          "branch_pages": 26658,
          "stored_leaf_bytes": 403788464,
          "metadata_bytes": 24769232,
          "fragmented_bytes": 426330752
        },
        {
          "table_name": "value_by_dict_pk",
          "table_entries": 1021784,
          "tree_height": 4,
          "leaf_pages": 82852,
          "branch_pages": 1294,
          "stored_leaf_bytes": 193033916,
          "metadata_bytes": 7111184,
          "fragmented_bytes": 144516916
        },
        {
          "table_name": "value_to_dict_pk",
          "table_entries": 1021784,
          "tree_height": 10,
          "leaf_pages": 71955,
          "branch_pages": 14149,
          "stored_leaf_bytes": 193033916,
          "metadata_bytes": 39630274,
          "fragmented_bytes": 120222594
        },
        {
          "table_name": "dict_pk_by_id",
          "table_entries": 50473558,
          "tree_height": 4,
          "leaf_pages": 394324,
          "branch_pages": 6160,
          "stored_leaf_bytes": 807576928,
          "metadata_bytes": 14392752,
          "fragmented_bytes": 818412784
        }
      ],
      "tree_template": [
        {
          "table_name": "dict_pk_to_ids",
          "table_entries": 50473558,
          "tree_height": 8,
          "leaf_pages": 179567,
          "branch_pages": 25847,
          "stored_leaf_bytes": 403788464,
          "metadata_bytes": 16720018,
          "fragmented_bytes": 420867262
        },
        {
          "table_name": "value_by_dict_pk",
          "table_entries": 555142,
          "tree_height": 4,
          "leaf_pages": 13083,
          "branch_pages": 205,
          "stored_leaf_bytes": 25177306,
          "metadata_bytes": 2698084,
          "fragmented_bytes": 26552258
        },
        {
          "table_name": "value_to_dict_pk",
          "table_entries": 555142,
          "tree_height": 7,
          "leaf_pages": 9702,
          "branch_pages": 484,
          "stored_leaf_bytes": 25177306,
          "metadata_bytes": 3408860,
          "fragmented_bytes": 13188938
        },
        {
          "table_name": "dict_pk_by_id",
          "table_entries": 50473558,
          "tree_height": 4,
          "leaf_pages": 394324,
          "branch_pages": 6160,
          "stored_leaf_bytes": 807576928,
          "metadata_bytes": 14392752,
          "fragmented_bytes": 818412784
        }
      ],
      "assets": {
        "id": {
          "table_name": "asset_id",
          "table_entries": 55470479,
          "tree_height": 4,
          "leaf_pages": 243291,
          "branch_pages": 3923,
          "stored_leaf_bytes": 499234311,
          "metadata_bytes": 9127270,
          "fragmented_bytes": 504226963
        },
        "amount": {
          "table_name": "asset_amount_by_id",
          "table_entries": 55470479,
          "tree_height": 4,
          "leaf_pages": 458433,
          "branch_pages": 7392,
          "stored_leaf_bytes": 942998143,
          "metadata_bytes": 17198532,
          "fragmented_bytes": 947822525
        },
        "name": [
          {
            "table_name": "dict_pk_to_ids",
            "table_entries": 55470479,
            "tree_height": 8,
            "leaf_pages": 228441,
            "branch_pages": 24380,
            "stored_leaf_bytes": 499234311,
            "metadata_bytes": 11666969,
            "fragmented_bytes": 524653536
          },
          {
            "table_name": "value_by_dict_pk",
            "table_entries": 136250,
            "tree_height": 3,
            "leaf_pages": 2961,
            "branch_pages": 48,
            "stored_leaf_bytes": 5586250,
            "metadata_bytes": 656060,
            "fragmented_bytes": 6082554
          },
          {
            "table_name": "value_to_dict_pk",
            "table_entries": 136250,
            "tree_height": 3,
            "leaf_pages": 2164,
            "branch_pages": 48,
            "stored_leaf_bytes": 5586250,
            "metadata_bytes": 684972,
            "fragmented_bytes": 2789130
          },
          {
            "table_name": "dict_pk_by_id",
            "table_entries": 55470479,
            "tree_height": 4,
            "leaf_pages": 486583,
            "branch_pages": 7846,
            "stored_leaf_bytes": 998468622,
            "metadata_bytes": 18254610,
            "fragmented_bytes": 1008457952
          }
        ],
        "asset_action": [
          {
            "table_name": "pk_by_index",
            "table_entries": 55470479,
            "tree_height": 5,
            "leaf_pages": 243291,
            "branch_pages": 3924,
            "stored_leaf_bytes": 499234311,
            "metadata_bytes": 9127312,
            "fragmented_bytes": 504231017
          },
          {
            "table_name": "index_by_pk",
            "table_entries": 55470479,
            "tree_height": 4,
            "leaf_pages": 270587,
            "branch_pages": 4364,
            "stored_leaf_bytes": 554704790,
            "metadata_bytes": 10151334,
            "fragmented_bytes": 561343172
          }
        ]
      }
    },
    "inputs": {
      "id": {
        "table_name": "input_id",
        "table_entries": 47016210,
        "tree_height": 4,
        "leaf_pages": 183657,
        "branch_pages": 2869,
        "stored_leaf_bytes": 376129680,
        "metadata_bytes": 6703428,
        "fragmented_bytes": 381177388
      },
      "utxo_pointer": {
        "table_name": "input_utxo_pointer_by_id",
        "table_entries": 47016210,
        "tree_height": 4,
        "leaf_pages": 367314,
        "branch_pages": 5738,
        "stored_leaf_bytes": 752259360,
        "metadata_bytes": 13406888,
        "fragmented_bytes": 762354744
      }
    }
  }
}
```