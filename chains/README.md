### ⏱ Redbit benchmarks (results from github servers)

The demo example persists data into 24 tables to allow for rich querying. Each `index` is backed by 2 tables and `dictionary` by 4 tables.
Each PK, FK, simple column, index or dictionary is backed by its own redb DB and a long-running indexing thread. If you have 20 of these, you are still
fine on Raspberry Pi, consider stronger machine for deeply nested entities with many indexes and dictionaries.

Indexing process is always as slow as the column which in comparison to others has either bigger values, more values or combination of both.

See [chain](../chain) for more details on performance and data size.

The `persist/remove` methods are slower because each bench iteration opens ~ 34 new databases for whole block.
The throughput is ~ **10 000 blocks/s** in batch mode which is ~ **300 000 db rows/s** until B+Tree grows significantly
=> write amplification increases and kernel page cache is fully utilized => kernel throttles writes.

The `block::_store_many` operation in this context writes and commits 3 blocks of 3 transactions of 1 input and 3 utxos of 3 assets, ie.
the operations writes :
- 3 blocks
- 3 * 3 = 9 transactions
- 3 * 3 = 9 inputs
- 3 * 3 * 3 = 27 utxos
- 3 * 3 * 3 * 3 = 81 assets

`block::_first` operation reads whole block with all its transactions, inputs, utxos and assets.

<!-- BEGIN_BENCH -->
```
function                                                          ops/s
-------------------------------------------------------------
model_v1::block_bench::_store_many                                  100
model_v1::block_bench::_persist                                     101
model_v1::block_bench::_remove                                      116
model_v1::block_bench::_store                                       118
model_v1::transaction_bench::_persist                               120
model_v1::transaction_bench::_store_many                            122
model_v1::transaction_bench::_store                                 125
model_v1::transaction_bench::_remove                                135
model_v1::utxo_bench::_store                                        230
model_v1::utxo_bench::_persist                                      244
model_v1::utxo_bench::_remove                                       254
model_v1::utxo_bench::_store_many                                   257
model_v1::input_bench::_store_many                                  360
model_v1::input_bench::_store                                       427
model_v1::header_bench::_persist                                    886
model_v1::header_bench::_remove                                    1475
model_v1::asset_bench::_persist                                    1887
model_v1::input_bench::_persist                                    2232
model_v1::maybevalue_bench::_persist                               2292
model_v1::asset_bench::_remove                                     2502
model_v1::input_bench::_remove                                     2658
model_v1::maybevalue_bench::_remove                                3030
model_v1::header_bench::_store_many                                3789
model_v1::header_bench::_store                                     4288
model_v1::block_bench::_take                                       4431
model_v1::block_bench::_tail                                       4534
model_v1::block_bench::_stream_range                               6294
model_v1::asset_bench::_store_many                                 6522
model_v1::transaction_bench::_stream_blocks_by_hash                6696
model_v1::asset_bench::_store                                      7228
model_v1::transaction_bench::_stream_range                         8691
model_v1::maybevalue_bench::_store_many                            9068
model_v1::block_bench::_get                                        9248
model_v1::block_bench::_first                                      9277
model_v1::block_bench::_get_transactions                           9305
model_v1::block_bench::_last                                       9381
model_v1::transaction_bench::_stream_by_hash                       9499
model_v1::maybevalue_bench::_store                                 9794
model_v1::utxo_bench::_stream_transactions_by_address             10660
model_v1::transaction_bench::_stream_ids_by_hash                  11734
model_v1::transaction_bench::_tail                                12709
model_v1::transaction_bench::_take                                14168
model_v1::utxo_bench::_stream_range                               19696
model_v1::utxo_bench::_stream_by_address                          20484
model_v1::asset_bench::_stream_utxos_by_name                      20647
model_v1::utxo_bench::_stream_ids_by_address                      23586
model_v1::transaction_bench::_get_by_hash                         29951
model_v1::transaction_bench::_first                               30081
model_v1::transaction_bench::_get                                 30103
model_v1::transaction_bench::_last                                30390
model_v1::block_bench::_range                                     33518
model_v1::header_bench::_stream_range_by_duration                 34539
model_v1::header_bench::_stream_range_by_timestamp                35151
model_v1::header_bench::_stream_range                             35983
model_v1::block_bench::_filter                                    37673
model_v1::header_bench::_stream_by_hash                           40272
model_v1::header_bench::_stream_by_prev_hash                      40331
model_v1::header_bench::_stream_by_duration                       40520
model_v1::transaction_bench::_range                               40585
model_v1::header_bench::_stream_by_timestamp                      40663
model_v1::header_bench::_stream_heights_by_duration               41108
model_v1::header_bench::_stream_heights_by_timestamp              41364
model_v1::header_bench::_stream_heights_by_hash                   41880
model_v1::header_bench::_stream_heights_by_prev_hash              41895
model_v1::asset_bench::_stream_range                              57396
model_v1::transaction_bench::_get_utxos                           64123
model_v1::transaction_bench::_filter                              65193
model_v1::asset_bench::_stream_by_name                            66528
model_v1::asset_bench::_stream_ids_by_name                        68992
model_v1::utxo_bench::_tail                                       90239
model_v1::utxo_bench::_take                                       91781
model_v1::input_bench::_stream_range                              92080
model_v1::maybevalue_bench::_stream_range                         95981
model_v1::maybevalue_bench::_stream_by_hash                      126554
model_v1::maybevalue_bench::_stream_ids_by_hash                  131335
model_v1::utxo_bench::_range                                     146135
model_v1::utxo_bench::_get_by_address                            219030
model_v1::utxo_bench::_first                                     235122
model_v1::utxo_bench::_get                                       237326
model_v1::utxo_bench::_last                                      238973
model_v1::utxo_bench::_filter                                    254646
model_v1::utxo_bench::_get_assets                                273254
model_v1::asset_bench::_tail                                     304214
model_v1::asset_bench::_range                                    309046
model_v1::header_bench::_range_by_duration                       320065
model_v1::header_bench::_tail                                    321709
model_v1::transaction_bench::_get_inputs                         323806
model_v1::input_bench::_range                                    326792
model_v1::header_bench::_range                                   330175
model_v1::input_bench::_tail                                     334971
model_v1::header_bench::_take                                    339457
model_v1::asset_bench::_take                                     344258
model_v1::header_bench::_range_by_timestamp                      349698
model_v1::maybevalue_bench::_range                               363420
model_v1::maybevalue_bench::_tail                                374469
model_v1::input_bench::_take                                     388098
model_v1::maybevalue_bench::_take                                423105
model_v1::asset_bench::_get_by_name                             1565460
model_v1::header_bench::_get_by_duration                        1693079
model_v1::header_bench::_get_by_prev_hash                       1820930
model_v1::header_bench::_get_by_hash                            1841722
model_v1::header_bench::_get_by_timestamp                       1899552
model_v1::asset_bench::_filter                                  2375128
model_v1::header_bench::_filter                                 2388915
model_v1::asset_bench::_get                                     2583779
model_v1::header_bench::_last                                   2658867
model_v1::header_bench::_first                                  2694038
model_v1::block_bench::_get_header                              2698181
model_v1::header_bench::_get                                    2766787
model_v1::asset_bench::_first                                   2772387
model_v1::asset_bench::_last                                    2783344
model_v1::maybevalue_bench::_get_by_hash                        3188674
model_v1::asset_bench::_get_ids_by_name                         3474514
model_v1::utxo_bench::_get_ids_by_address                       3519887
model_v1::header_bench::_get_heights_by_duration                3884099
model_v1::input_bench::_get                                     4126945
model_v1::input_bench::_filter                                  4128819
model_v1::input_bench::_last                                    4374262
model_v1::input_bench::_first                                   4440300
model_v1::transaction_bench::_get_ids_by_hash                   4976115
model_v1::maybevalue_bench::_get_ids_by_hash                    5122688
model_v1::header_bench::_get_heights_by_hash                    5174912
model_v1::header_bench::_get_heights_by_timestamp               5258453
model_v1::header_bench::_get_heights_by_prev_hash               5273705
model_v1::maybevalue_bench::_filter                             6366994
model_v1::maybevalue_bench::_last                               6766816
model_v1::maybevalue_bench::_first                              6818957
model_v1::maybevalue_bench::_get                                6870963
model_v1::transaction_bench::_get_maybe                         7032843
model_v1::asset_bench::_exists                                  9979044
model_v1::utxo_bench::_exists                                  12594458
model_v1::input_bench::_exists                                 12621482
model_v1::transaction_bench::_exists                           12730745
model_v1::maybevalue_bench::_exists                            13312034
model_v1::block_bench::_exists                                 16823688
model_v1::header_bench::_exists                                16866251
```
<!-- END_BENCH -->

