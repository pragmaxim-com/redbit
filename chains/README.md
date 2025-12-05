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
model_v1::block_bench::_store_many                                   68
model_v1::block_bench::_persist                                      82
model_v1::block_bench::_store                                        93
model_v1::transaction_bench::_store_many                             99
model_v1::transaction_bench::_store                                 107
model_v1::transaction_bench::_persist                               108
model_v1::block_bench::_remove                                      113
model_v1::transaction_bench::_remove                                124
model_v1::utxo_bench::_store                                        202
model_v1::utxo_bench::_store_many                                   215
model_v1::utxo_bench::_persist                                      230
model_v1::utxo_bench::_remove                                       237
model_v1::input_bench::_store_many                                  456
model_v1::input_bench::_store                                       460
model_v1::header_bench::_persist                                    878
model_v1::asset_bench::_persist                                    1858
model_v1::header_bench::_remove                                    1925
model_v1::input_bench::_persist                                    2647
model_v1::maybevalue_bench::_persist                               2773
model_v1::asset_bench::_remove                                     3014
model_v1::input_bench::_remove                                     3362
model_v1::maybevalue_bench::_remove                                3807
model_v1::header_bench::_store_many                                5166
model_v1::header_bench::_store                                     5469
model_v1::block_bench::_tail                                       5538
model_v1::block_bench::_take                                       5539
model_v1::block_bench::_stream_range                               8270
model_v1::asset_bench::_store_many                                 8466
model_v1::asset_bench::_store                                      8477
model_v1::transaction_bench::_stream_blocks_by_hash                9337
model_v1::block_bench::_first                                     11499
model_v1::block_bench::_get                                       11539
model_v1::block_bench::_get_transactions                          11561
model_v1::maybevalue_bench::_store                                11626
model_v1::block_bench::_last                                      11728
model_v1::maybevalue_bench::_store_many                           12057
model_v1::transaction_bench::_stream_range                        12375
model_v1::transaction_bench::_stream_by_hash                      13571
model_v1::utxo_bench::_stream_transactions_by_address             15080
model_v1::transaction_bench::_stream_ids_by_hash                  17007
model_v1::transaction_bench::_tail                                17720
model_v1::transaction_bench::_take                                17949
model_v1::utxo_bench::_stream_range                               27244
model_v1::utxo_bench::_stream_by_address                          27617
model_v1::asset_bench::_stream_utxos_by_name                      29486
model_v1::utxo_bench::_stream_ids_by_address                      32209
model_v1::transaction_bench::_get_by_hash                         36469
model_v1::transaction_bench::_first                               36584
model_v1::transaction_bench::_get                                 36808
model_v1::transaction_bench::_last                                36927
model_v1::block_bench::_range                                     42261
model_v1::block_bench::_filter                                    47148
model_v1::header_bench::_stream_range_by_duration                 48817
model_v1::header_bench::_stream_range_by_timestamp                49639
model_v1::transaction_bench::_range                               49896
model_v1::header_bench::_stream_range                             51421
model_v1::header_bench::_stream_by_duration                       54317
model_v1::header_bench::_stream_by_timestamp                      56059
model_v1::header_bench::_stream_by_prev_hash                      56470
model_v1::header_bench::_stream_by_hash                           56797
model_v1::header_bench::_stream_heights_by_prev_hash              57947
model_v1::header_bench::_stream_heights_by_duration               58025
model_v1::header_bench::_stream_heights_by_timestamp              58033
model_v1::header_bench::_stream_heights_by_hash                   58133
model_v1::asset_bench::_stream_range                              76811
model_v1::transaction_bench::_filter                              78601
model_v1::transaction_bench::_get_utxos                           79580
model_v1::asset_bench::_stream_by_name                            82538
model_v1::asset_bench::_stream_ids_by_name                        94330
model_v1::utxo_bench::_tail                                      117003
model_v1::utxo_bench::_take                                      121459
model_v1::input_bench::_stream_range                             122139
model_v1::maybevalue_bench::_stream_range                        124412
model_v1::maybevalue_bench::_stream_by_hash                      164885
model_v1::maybevalue_bench::_stream_ids_by_hash                  166318
model_v1::utxo_bench::_range                                     187217
model_v1::utxo_bench::_get_by_address                            286869
model_v1::utxo_bench::_first                                     307092
model_v1::utxo_bench::_get                                       307317
model_v1::utxo_bench::_last                                      316006
model_v1::utxo_bench::_filter                                    324034
model_v1::utxo_bench::_get_assets                                363511
model_v1::asset_bench::_tail                                     404919
model_v1::transaction_bench::_get_inputs                         410560
model_v1::asset_bench::_range                                    412723
model_v1::input_bench::_range                                    434501
model_v1::header_bench::_tail                                    454727
model_v1::header_bench::_range_by_duration                       467410
model_v1::header_bench::_range                                   471171
model_v1::maybevalue_bench::_range                               475095
model_v1::asset_bench::_take                                     475254
model_v1::input_bench::_tail                                     481559
model_v1::header_bench::_take                                    491316
model_v1::header_bench::_range_by_timestamp                      501995
model_v1::maybevalue_bench::_tail                                518226
model_v1::input_bench::_take                                     564051
model_v1::maybevalue_bench::_take                                609318
model_v1::asset_bench::_get_by_name                             1623034
model_v1::header_bench::_get_by_duration                        1877652
model_v1::header_bench::_get_by_prev_hash                       2035831
model_v1::header_bench::_get_by_hash                            2052545
model_v1::header_bench::_get_by_timestamp                       2098592
model_v1::asset_bench::_filter                                  2470417
model_v1::asset_bench::_get                                     2661202
model_v1::header_bench::_filter                                 2734332
model_v1::asset_bench::_first                                   2853393
model_v1::asset_bench::_last                                    2878609
model_v1::header_bench::_first                                  2990967
model_v1::header_bench::_last                                   2993743
model_v1::block_bench::_get_header                              3003003
model_v1::header_bench::_get                                    3115265
model_v1::asset_bench::_get_ids_by_name                         3448038
model_v1::maybevalue_bench::_get_by_hash                        3624502
model_v1::input_bench::_filter                                  3643385
model_v1::utxo_bench::_get_ids_by_address                       3651501
model_v1::input_bench::_get                                     3736921
model_v1::input_bench::_first                                   3929273
model_v1::input_bench::_last                                    3929427
model_v1::header_bench::_get_heights_by_duration                4293504
model_v1::header_bench::_get_heights_by_prev_hash               4984051
model_v1::header_bench::_get_heights_by_hash                    4985045
model_v1::transaction_bench::_get_ids_by_hash                   4993758
model_v1::header_bench::_get_heights_by_timestamp               5193996
model_v1::maybevalue_bench::_get_ids_by_hash                    5407159
model_v1::maybevalue_bench::_filter                             6153467
model_v1::maybevalue_bench::_get                                6269986
model_v1::transaction_bench::_get_maybe                         6309148
model_v1::maybevalue_bench::_last                               6554368
model_v1::maybevalue_bench::_first                              6571598
model_v1::asset_bench::_exists                                 10253255
model_v1::transaction_bench::_exists                           11286682
model_v1::input_bench::_exists                                 11600928
model_v1::utxo_bench::_exists                                  11646867
model_v1::maybevalue_bench::_exists                            11996161
model_v1::header_bench::_exists                                18248175
model_v1::block_bench::_exists                                 18258171
```
<!-- END_BENCH -->

