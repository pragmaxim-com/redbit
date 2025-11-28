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
model_v1::block_bench::_store_many                                   82
model_v1::block_bench::_persist                                      85
model_v1::transaction_bench::_store_many                             98
model_v1::block_bench::_store                                       103
model_v1::transaction_bench::_persist                               105
model_v1::transaction_bench::_store                                 107
model_v1::block_bench::_remove                                      108
model_v1::transaction_bench::_remove                                117
model_v1::utxo_bench::_store                                        204
model_v1::utxo_bench::_store_many                                   218
model_v1::utxo_bench::_persist                                      223
model_v1::utxo_bench::_remove                                       233
model_v1::input_bench::_store                                       404
model_v1::input_bench::_store_many                                  411
model_v1::header_bench::_persist                                    883
model_v1::header_bench::_remove                                    1460
model_v1::asset_bench::_persist                                    1823
model_v1::input_bench::_persist                                    2161
model_v1::maybevalue_bench::_persist                               2269
model_v1::asset_bench::_remove                                     2461
model_v1::input_bench::_remove                                     2617
model_v1::maybevalue_bench::_remove                                2917
model_v1::header_bench::_store_many                                4103
model_v1::block_bench::_take                                       4256
model_v1::block_bench::_tail                                       4260
model_v1::header_bench::_store                                     4299
model_v1::block_bench::_stream_range                               6083
model_v1::asset_bench::_store_many                                 6563
model_v1::transaction_bench::_stream_blocks_by_hash                6670
model_v1::asset_bench::_store                                      7063
model_v1::block_bench::_last                                       8714
model_v1::block_bench::_first                                      8750
model_v1::transaction_bench::_stream_range                         8797
model_v1::block_bench::_get                                        8996
model_v1::block_bench::_get_transactions                           9045
model_v1::maybevalue_bench::_store_many                            9156
model_v1::transaction_bench::_stream_by_hash                       9510
model_v1::maybevalue_bench::_store                                 9590
model_v1::utxo_bench::_stream_transactions_by_address             10649
model_v1::transaction_bench::_stream_ids_by_hash                  11680
model_v1::transaction_bench::_tail                                14453
model_v1::transaction_bench::_take                                14457
model_v1::utxo_bench::_stream_range                               19390
model_v1::utxo_bench::_stream_by_address                          21033
model_v1::asset_bench::_stream_utxos_by_name                      21820
model_v1::utxo_bench::_stream_ids_by_address                      23427
model_v1::transaction_bench::_get_by_hash                         29650
model_v1::transaction_bench::_get                                 29825
model_v1::transaction_bench::_first                               29941
model_v1::transaction_bench::_last                                30146
model_v1::block_bench::_range                                     32597
model_v1::header_bench::_stream_range_by_duration                 34993
model_v1::header_bench::_stream_range_by_timestamp                35080
model_v1::header_bench::_stream_range                             36282
model_v1::block_bench::_filter                                    37473
model_v1::header_bench::_stream_by_hash                           39849
model_v1::transaction_bench::_range                               39937
model_v1::header_bench::_stream_by_prev_hash                      40084
model_v1::header_bench::_stream_by_timestamp                      40256
model_v1::header_bench::_stream_heights_by_timestamp              41039
model_v1::header_bench::_stream_heights_by_prev_hash              41255
model_v1::header_bench::_stream_heights_by_duration               41286
model_v1::header_bench::_stream_by_duration                       41326
model_v1::header_bench::_stream_heights_by_hash                   41397
model_v1::asset_bench::_stream_range                              59540
model_v1::transaction_bench::_get_utxos                           63388
model_v1::transaction_bench::_filter                              65163
model_v1::asset_bench::_stream_by_name                            69883
model_v1::asset_bench::_stream_ids_by_name                        72671
model_v1::utxo_bench::_take                                       73808
model_v1::input_bench::_stream_range                              90531
model_v1::utxo_bench::_tail                                       90820
model_v1::maybevalue_bench::_stream_range                         92434
model_v1::maybevalue_bench::_stream_by_hash                      122816
model_v1::maybevalue_bench::_stream_ids_by_hash                  126538
model_v1::utxo_bench::_range                                     145265
model_v1::utxo_bench::_get_by_address                            217795
model_v1::utxo_bench::_get                                       237362
model_v1::utxo_bench::_first                                     237561
model_v1::utxo_bench::_last                                      240036
model_v1::utxo_bench::_filter                                    256553
model_v1::utxo_bench::_get_assets                                267797
model_v1::asset_bench::_tail                                     302123
model_v1::asset_bench::_range                                    315428
model_v1::input_bench::_range                                    318899
model_v1::transaction_bench::_get_inputs                         321562
model_v1::header_bench::_range_by_duration                       330120
model_v1::header_bench::_tail                                    332805
model_v1::header_bench::_range                                   339356
model_v1::maybevalue_bench::_range                               343091
model_v1::input_bench::_tail                                     346312
model_v1::asset_bench::_take                                     349386
model_v1::header_bench::_take                                    350716
model_v1::header_bench::_range_by_timestamp                      355726
model_v1::maybevalue_bench::_tail                                359153
model_v1::input_bench::_take                                     390529
model_v1::maybevalue_bench::_take                                408482
model_v1::asset_bench::_get_by_name                             1562134
model_v1::header_bench::_get_by_duration                        1687450
model_v1::header_bench::_get_by_hash                            1857735
model_v1::header_bench::_get_by_prev_hash                       1937008
model_v1::header_bench::_get_by_timestamp                       1962901
model_v1::header_bench::_filter                                 2357990
model_v1::asset_bench::_filter                                  2363452
model_v1::asset_bench::_get                                     2534726
model_v1::header_bench::_get                                    2657031
model_v1::block_bench::_get_header                              2680462
model_v1::header_bench::_last                                   2702776
model_v1::header_bench::_first                                  2763576
model_v1::asset_bench::_first                                   2774002
model_v1::asset_bench::_last                                    2831017
model_v1::maybevalue_bench::_get_by_hash                        3044696
model_v1::asset_bench::_get_ids_by_name                         3508526
model_v1::utxo_bench::_get_ids_by_address                       3604253
model_v1::header_bench::_get_heights_by_duration                4042691
model_v1::input_bench::_filter                                  4071661
model_v1::input_bench::_get                                     4168925
model_v1::header_bench::_get_heights_by_hash                    4188482
model_v1::header_bench::_get_heights_by_prev_hash               4324137
model_v1::input_bench::_last                                    4478882
model_v1::maybevalue_bench::_get_ids_by_hash                    4499235
model_v1::input_bench::_first                                   4526321
model_v1::transaction_bench::_get_ids_by_hash                   4955156
model_v1::header_bench::_get_heights_by_timestamp               5250998
model_v1::maybevalue_bench::_filter                             6457445
model_v1::maybevalue_bench::_get                                6480041
model_v1::transaction_bench::_get_maybe                         6679581
model_v1::maybevalue_bench::_first                              6807815
model_v1::maybevalue_bench::_last                               6922811
model_v1::asset_bench::_exists                                  9135757
model_v1::utxo_bench::_exists                                  11810559
model_v1::input_bench::_exists                                 12087514
model_v1::maybevalue_bench::_exists                            12901561
model_v1::transaction_bench::_exists                           12904891
model_v1::header_bench::_exists                                14757969
model_v1::block_bench::_exists                                 16937669
```
<!-- END_BENCH -->

