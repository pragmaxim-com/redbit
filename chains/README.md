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
model_v1::block_bench::_store_many                                   99
model_v1::block_bench::_persist                                     101
model_v1::block_bench::_store                                       123
model_v1::block_bench::_remove                                      125
model_v1::transaction_bench::_persist                               127
model_v1::transaction_bench::_store_many                            127
model_v1::transaction_bench::_store                                 132
model_v1::transaction_bench::_remove                                140
model_v1::utxo_bench::_store                                        247
model_v1::utxo_bench::_persist                                      262
model_v1::utxo_bench::_remove                                       263
model_v1::utxo_bench::_store_many                                   264
model_v1::input_bench::_store_many                                  375
model_v1::input_bench::_store                                       454
model_v1::header_bench::_persist                                    884
model_v1::header_bench::_remove                                    1330
model_v1::asset_bench::_persist                                    1604
model_v1::input_bench::_persist                                    1891
model_v1::maybevalue_bench::_persist                               1966
model_v1::input_bench::_remove                                     2225
model_v1::asset_bench::_remove                                     2260
model_v1::maybevalue_bench::_remove                                2444
model_v1::header_bench::_store_many                                4103
model_v1::block_bench::_take                                       4287
model_v1::block_bench::_tail                                       4302
model_v1::header_bench::_store                                     4398
model_v1::block_bench::_stream_range                               6275
model_v1::transaction_bench::_stream_blocks_by_hash                6540
model_v1::asset_bench::_store_many                                 6750
model_v1::asset_bench::_store                                      7188
model_v1::transaction_bench::_stream_range                         8679
model_v1::block_bench::_first                                      8748
model_v1::block_bench::_get                                        8753
model_v1::block_bench::_get_transactions                           8886
model_v1::block_bench::_last                                       8972
model_v1::maybevalue_bench::_store                                 9209
model_v1::transaction_bench::_stream_by_hash                       9496
model_v1::maybevalue_bench::_store_many                            9865
model_v1::utxo_bench::_stream_transactions_by_address             10699
model_v1::transaction_bench::_stream_ids_by_hash                  11693
model_v1::transaction_bench::_tail                                12452
model_v1::transaction_bench::_take                                13333
model_v1::utxo_bench::_stream_range                               19525
model_v1::utxo_bench::_stream_by_address                          20593
model_v1::asset_bench::_stream_utxos_by_name                      21818
model_v1::utxo_bench::_stream_ids_by_address                      23352
model_v1::transaction_bench::_get_by_hash                         28049
model_v1::transaction_bench::_first                               28441
model_v1::transaction_bench::_last                                28461
model_v1::transaction_bench::_get                                 28481
model_v1::block_bench::_range                                     32378
model_v1::header_bench::_stream_range_by_duration                 34490
model_v1::header_bench::_stream_range_by_timestamp                35046
model_v1::header_bench::_stream_range                             36102
model_v1::block_bench::_filter                                    36348
model_v1::transaction_bench::_range                               37742
model_v1::header_bench::_stream_by_duration                       39574
model_v1::header_bench::_stream_by_hash                           39860
model_v1::header_bench::_stream_by_timestamp                      39866
model_v1::header_bench::_stream_by_prev_hash                      40146
model_v1::header_bench::_stream_heights_by_timestamp              40222
model_v1::header_bench::_stream_heights_by_hash                   40881
model_v1::header_bench::_stream_heights_by_duration               41175
model_v1::header_bench::_stream_heights_by_prev_hash              41202
model_v1::asset_bench::_stream_range                              57925
model_v1::transaction_bench::_filter                              61079
model_v1::transaction_bench::_get_utxos                           61318
model_v1::asset_bench::_stream_by_name                            64065
model_v1::asset_bench::_stream_ids_by_name                        71308
model_v1::utxo_bench::_tail                                       87781
model_v1::maybevalue_bench::_stream_range                         90757
model_v1::utxo_bench::_take                                       90826
model_v1::input_bench::_stream_range                              93470
model_v1::maybevalue_bench::_stream_ids_by_hash                  121900
model_v1::maybevalue_bench::_stream_by_hash                      122333
model_v1::utxo_bench::_range                                     141737
model_v1::utxo_bench::_get_by_address                            210648
model_v1::utxo_bench::_first                                     227607
model_v1::utxo_bench::_get                                       230423
model_v1::utxo_bench::_last                                      231454
model_v1::utxo_bench::_filter                                    246312
model_v1::utxo_bench::_get_assets                                263812
model_v1::asset_bench::_range                                    298647
model_v1::asset_bench::_tail                                     301877
model_v1::header_bench::_tail                                    321239
model_v1::transaction_bench::_get_inputs                         323218
model_v1::header_bench::_range_by_duration                       331333
model_v1::input_bench::_range                                    337540
model_v1::header_bench::_range                                   338721
model_v1::maybevalue_bench::_range                               342278
model_v1::header_bench::_take                                    346672
model_v1::asset_bench::_take                                     349834
model_v1::maybevalue_bench::_tail                                354298
model_v1::input_bench::_tail                                     354545
model_v1::header_bench::_range_by_timestamp                      359800
model_v1::input_bench::_take                                     399487
model_v1::maybevalue_bench::_take                                401445
model_v1::asset_bench::_get_by_name                             1450831
model_v1::header_bench::_get_by_duration                        1718036
model_v1::header_bench::_get_by_timestamp                       1834458
model_v1::header_bench::_get_by_prev_hash                       1853156
model_v1::header_bench::_get_by_hash                            1857769
model_v1::asset_bench::_get                                     2103580
model_v1::asset_bench::_filter                                  2195534
model_v1::header_bench::_filter                                 2285505
model_v1::asset_bench::_first                                   2482930
model_v1::asset_bench::_last                                    2531389
model_v1::block_bench::_get_header                              2676086
model_v1::header_bench::_last                                   2722274
model_v1::header_bench::_first                                  2726876
model_v1::header_bench::_get                                    2804341
model_v1::maybevalue_bench::_get_by_hash                        3125586
model_v1::asset_bench::_get_ids_by_name                         3469090
model_v1::utxo_bench::_get_ids_by_address                       3677011
model_v1::header_bench::_get_heights_by_duration                4054657
model_v1::input_bench::_filter                                  4103574
model_v1::input_bench::_get                                     4431642
model_v1::transaction_bench::_get_ids_by_hash                   4608507
model_v1::input_bench::_last                                    4634350
model_v1::input_bench::_first                                   4643603
model_v1::maybevalue_bench::_get_ids_by_hash                    4746760
model_v1::header_bench::_get_heights_by_timestamp               5157032
model_v1::header_bench::_get_heights_by_hash                    5400734
model_v1::header_bench::_get_heights_by_prev_hash               5423582
model_v1::maybevalue_bench::_filter                             6251563
model_v1::transaction_bench::_get_maybe                         6500683
model_v1::maybevalue_bench::_get                                6558237
model_v1::maybevalue_bench::_last                               6651590
model_v1::maybevalue_bench::_first                              6708708
model_v1::asset_bench::_exists                                  9501188
model_v1::maybevalue_bench::_exists                            11897680
model_v1::transaction_bench::_exists                           11934598
model_v1::input_bench::_exists                                 12272950
model_v1::utxo_bench::_exists                                  12496876
model_v1::header_bench::_exists                                16490765
model_v1::block_bench::_exists                                 16863406
```
<!-- END_BENCH -->

