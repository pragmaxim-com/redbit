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
model_v1::block_bench::_store_many                                   70
model_v1::block_bench::_persist                                      77
model_v1::transaction_bench::_store_many                             99
model_v1::block_bench::_store                                       100
model_v1::transaction_bench::_persist                               102
model_v1::block_bench::_remove                                      106
model_v1::transaction_bench::_store                                 109
model_v1::transaction_bench::_remove                                110
model_v1::utxo_bench::_store                                        210
model_v1::utxo_bench::_store_many                                   220
model_v1::utxo_bench::_persist                                      226
model_v1::utxo_bench::_remove                                       239
model_v1::input_bench::_store                                       477
model_v1::input_bench::_store_many                                  484
model_v1::header_bench::_persist                                    876
model_v1::asset_bench::_persist                                    1786
model_v1::header_bench::_remove                                    1802
model_v1::input_bench::_persist                                    2655
model_v1::maybevalue_bench::_persist                               2863
model_v1::asset_bench::_remove                                     2874
model_v1::input_bench::_remove                                     3165
model_v1::maybevalue_bench::_remove                                3616
model_v1::header_bench::_store_many                                5097
model_v1::block_bench::_take                                       5351
model_v1::header_bench::_store                                     5354
model_v1::block_bench::_tail                                       5412
model_v1::asset_bench::_store_many                                 8124
model_v1::asset_bench::_store                                      8316
model_v1::block_bench::_stream_range                               8463
model_v1::transaction_bench::_stream_blocks_by_hash                9470
model_v1::block_bench::_first                                     10709
model_v1::block_bench::_get_transactions                          10726
model_v1::block_bench::_get                                       10798
model_v1::block_bench::_last                                      10826
model_v1::maybevalue_bench::_store                                12099
model_v1::maybevalue_bench::_store_many                           12149
model_v1::transaction_bench::_stream_range                        12825
model_v1::transaction_bench::_stream_by_hash                      13899
model_v1::utxo_bench::_stream_transactions_by_address             14837
model_v1::transaction_bench::_stream_ids_by_hash                  17623
model_v1::transaction_bench::_tail                                17850
model_v1::transaction_bench::_take                                17948
model_v1::utxo_bench::_stream_range                               26348
model_v1::utxo_bench::_stream_by_address                          27494
model_v1::asset_bench::_stream_utxos_by_name                      29331
model_v1::utxo_bench::_stream_ids_by_address                      31051
model_v1::transaction_bench::_get                                 37062
model_v1::transaction_bench::_get_by_hash                         37294
model_v1::transaction_bench::_first                               37578
model_v1::transaction_bench::_last                                37635
model_v1::block_bench::_range                                     43149
model_v1::block_bench::_filter                                    45405
model_v1::header_bench::_stream_range_by_duration                 48424
model_v1::header_bench::_stream_range_by_timestamp                49135
model_v1::transaction_bench::_range                               49151
model_v1::header_bench::_stream_range                             50147
model_v1::header_bench::_stream_by_hash                           55145
model_v1::header_bench::_stream_by_duration                       55295
model_v1::header_bench::_stream_by_prev_hash                      55486
model_v1::header_bench::_stream_by_timestamp                      55852
model_v1::header_bench::_stream_heights_by_duration               57882
model_v1::header_bench::_stream_heights_by_timestamp              57990
model_v1::header_bench::_stream_heights_by_hash                   58059
model_v1::header_bench::_stream_heights_by_prev_hash              58349
model_v1::asset_bench::_stream_range                              79001
model_v1::transaction_bench::_get_utxos                           80476
model_v1::transaction_bench::_filter                              81828
model_v1::asset_bench::_stream_by_name                            87492
model_v1::asset_bench::_stream_ids_by_name                        98009
model_v1::utxo_bench::_tail                                      112093
model_v1::input_bench::_stream_range                             117015
model_v1::utxo_bench::_take                                      119304
model_v1::maybevalue_bench::_stream_range                        126201
model_v1::maybevalue_bench::_stream_by_hash                      164926
model_v1::maybevalue_bench::_stream_ids_by_hash                  168926
model_v1::utxo_bench::_range                                     191638
model_v1::utxo_bench::_get_by_address                            286419
model_v1::utxo_bench::_get                                       303370
model_v1::utxo_bench::_first                                     306839
model_v1::utxo_bench::_last                                      314999
model_v1::utxo_bench::_filter                                    340490
model_v1::utxo_bench::_get_assets                                362589
model_v1::asset_bench::_tail                                     384383
model_v1::transaction_bench::_get_inputs                         404098
model_v1::asset_bench::_range                                    415920
model_v1::input_bench::_range                                    430091
model_v1::header_bench::_tail                                    458703
model_v1::input_bench::_tail                                     467604
model_v1::asset_bench::_take                                     469404
model_v1::header_bench::_range                                   485857
model_v1::header_bench::_range_by_duration                       488677
model_v1::header_bench::_take                                    490769
model_v1::maybevalue_bench::_range                               499184
model_v1::maybevalue_bench::_tail                                516724
model_v1::header_bench::_range_by_timestamp                      519556
model_v1::input_bench::_take                                     553382
model_v1::maybevalue_bench::_take                                615908
model_v1::asset_bench::_get_by_name                             1616946
model_v1::header_bench::_get_by_duration                        1862510
model_v1::header_bench::_get_by_timestamp                       2051998
model_v1::header_bench::_get_by_hash                            2083724
model_v1::header_bench::_get_by_prev_hash                       2092970
model_v1::asset_bench::_filter                                  2393547
model_v1::asset_bench::_get                                     2700805
model_v1::header_bench::_filter                                 2727174
model_v1::asset_bench::_last                                    2798534
model_v1::asset_bench::_first                                   2813098
model_v1::header_bench::_last                                   2933153
model_v1::header_bench::_first                                  2953424
model_v1::block_bench::_get_header                              3062318
model_v1::header_bench::_get                                    3140901
model_v1::maybevalue_bench::_get_by_hash                        3368818
model_v1::asset_bench::_get_ids_by_name                         3609717
model_v1::utxo_bench::_get_ids_by_address                       3669186
model_v1::input_bench::_filter                                  3676200
model_v1::input_bench::_get                                     3718301
model_v1::input_bench::_first                                   3972510
model_v1::input_bench::_last                                    3972668
model_v1::header_bench::_get_heights_by_duration                3994408
model_v1::header_bench::_get_heights_by_timestamp               4859323
model_v1::maybevalue_bench::_get_ids_by_hash                    5007762
model_v1::header_bench::_get_heights_by_hash                    5119541
model_v1::header_bench::_get_heights_by_prev_hash               5183227
model_v1::transaction_bench::_get_ids_by_hash                   5266207
model_v1::maybevalue_bench::_get                                5997001
model_v1::maybevalue_bench::_filter                             6155361
model_v1::transaction_bench::_get_maybe                         6199244
model_v1::maybevalue_bench::_first                              6673340
model_v1::maybevalue_bench::_last                               6695233
model_v1::asset_bench::_exists                                 10322048
model_v1::input_bench::_exists                                 11633318
model_v1::utxo_bench::_exists                                  11706860
model_v1::maybevalue_bench::_exists                            11827321
model_v1::transaction_bench::_exists                           11852554
model_v1::block_bench::_exists                                 17746229
model_v1::header_bench::_exists                                18419598
```
<!-- END_BENCH -->

