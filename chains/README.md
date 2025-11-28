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
model_v1::block_bench::_store_many                                  104
model_v1::block_bench::_persist                                     106
model_v1::block_bench::_store                                       127
model_v1::block_bench::_remove                                      130
model_v1::transaction_bench::_store_many                            131
model_v1::transaction_bench::_persist                               133
model_v1::transaction_bench::_store                                 137
model_v1::transaction_bench::_remove                                146
model_v1::utxo_bench::_store                                        260
model_v1::utxo_bench::_store_many                                   270
model_v1::utxo_bench::_persist                                      272
model_v1::utxo_bench::_remove                                       283
model_v1::input_bench::_store                                       455
model_v1::input_bench::_store_many                                  456
model_v1::header_bench::_persist                                    880
model_v1::header_bench::_remove                                    1632
model_v1::asset_bench::_persist                                    1871
model_v1::input_bench::_persist                                    2424
model_v1::asset_bench::_remove                                     2576
model_v1::maybevalue_bench::_persist                               2612
model_v1::input_bench::_remove                                     2819
model_v1::maybevalue_bench::_remove                                3247
model_v1::header_bench::_store_many                                3718
model_v1::header_bench::_store                                     3866
model_v1::block_bench::_take                                       4458
model_v1::block_bench::_tail                                       4528
model_v1::block_bench::_stream_range                               6219
model_v1::asset_bench::_store_many                                 6470
model_v1::transaction_bench::_stream_blocks_by_hash                6763
model_v1::asset_bench::_store                                      7270
model_v1::transaction_bench::_stream_range                         8770
model_v1::block_bench::_first                                      9262
model_v1::block_bench::_get                                        9284
model_v1::transaction_bench::_stream_by_hash                       9312
model_v1::maybevalue_bench::_store_many                            9347
model_v1::block_bench::_get_transactions                           9377
model_v1::block_bench::_last                                       9454
model_v1::maybevalue_bench::_store                                 9765
model_v1::utxo_bench::_stream_transactions_by_address             11050
model_v1::transaction_bench::_stream_ids_by_hash                  11916
model_v1::transaction_bench::_take                                12186
model_v1::transaction_bench::_tail                                13069
model_v1::utxo_bench::_stream_range                               19088
model_v1::utxo_bench::_stream_by_address                          19699
model_v1::asset_bench::_stream_utxos_by_name                      21930
model_v1::utxo_bench::_stream_ids_by_address                      22359
model_v1::transaction_bench::_get_by_hash                         29419
model_v1::transaction_bench::_get                                 29841
model_v1::transaction_bench::_first                               29927
model_v1::transaction_bench::_last                                30059
model_v1::header_bench::_stream_range_by_duration                 33457
model_v1::block_bench::_range                                     34305
model_v1::header_bench::_stream_range                             34434
model_v1::header_bench::_stream_range_by_timestamp                35410
model_v1::block_bench::_filter                                    38254
model_v1::header_bench::_stream_heights_by_prev_hash              39453
model_v1::header_bench::_stream_by_duration                       40040
model_v1::header_bench::_stream_by_prev_hash                      40055
model_v1::transaction_bench::_range                               40085
model_v1::header_bench::_stream_by_hash                           40162
model_v1::header_bench::_stream_by_timestamp                      40327
model_v1::header_bench::_stream_heights_by_timestamp              40887
model_v1::header_bench::_stream_heights_by_hash                   41281
model_v1::header_bench::_stream_heights_by_duration               41538
model_v1::asset_bench::_stream_range                              57959
model_v1::transaction_bench::_get_utxos                           64121
model_v1::transaction_bench::_filter                              64416
model_v1::asset_bench::_stream_by_name                            69487
model_v1::asset_bench::_stream_ids_by_name                        71840
model_v1::input_bench::_stream_range                              91440
model_v1::utxo_bench::_tail                                       91478
model_v1::maybevalue_bench::_stream_range                         93444
model_v1::utxo_bench::_take                                       94423
model_v1::maybevalue_bench::_stream_by_hash                      123168
model_v1::maybevalue_bench::_stream_ids_by_hash                  126524
model_v1::utxo_bench::_range                                     144718
model_v1::utxo_bench::_get_by_address                            223755
model_v1::utxo_bench::_first                                     236312
model_v1::utxo_bench::_get                                       240566
model_v1::utxo_bench::_last                                      242787
model_v1::utxo_bench::_filter                                    256823
model_v1::utxo_bench::_get_assets                                276434
model_v1::asset_bench::_tail                                     307295
model_v1::asset_bench::_range                                    310968
model_v1::transaction_bench::_get_inputs                         320778
model_v1::header_bench::_range_by_duration                       325730
model_v1::header_bench::_tail                                    328048
model_v1::input_bench::_range                                    334765
model_v1::input_bench::_tail                                     336597
model_v1::asset_bench::_take                                     337317
model_v1::header_bench::_range                                   342538
model_v1::header_bench::_take                                    345823
model_v1::maybevalue_bench::_range                               360354
model_v1::header_bench::_range_by_timestamp                      364979
model_v1::maybevalue_bench::_tail                                373536
model_v1::input_bench::_take                                     381901
model_v1::maybevalue_bench::_take                                419184
model_v1::header_bench::_get_by_duration                        1632920
model_v1::asset_bench::_get_by_name                             1656617
model_v1::header_bench::_get_by_timestamp                       1869683
model_v1::header_bench::_get_by_prev_hash                       1874731
model_v1::header_bench::_get_by_hash                            1880088
model_v1::header_bench::_filter                                 2378687
model_v1::asset_bench::_filter                                  2473962
model_v1::block_bench::_get_header                              2563511
model_v1::asset_bench::_get                                     2596863
model_v1::header_bench::_last                                   2726430
model_v1::header_bench::_first                                  2761363
model_v1::header_bench::_get                                    2824061
model_v1::asset_bench::_first                                   2830376
model_v1::asset_bench::_last                                    2848841
model_v1::utxo_bench::_get_ids_by_address                       3200819
model_v1::asset_bench::_get_ids_by_name                         3387304
model_v1::maybevalue_bench::_get_by_hash                        3429473
model_v1::header_bench::_get_heights_by_duration                4064215
model_v1::input_bench::_filter                                  4239444
model_v1::input_bench::_get                                     4331067
model_v1::input_bench::_first                                   4479283
model_v1::input_bench::_last                                    4479885
model_v1::transaction_bench::_get_ids_by_hash                   4557885
model_v1::maybevalue_bench::_get_ids_by_hash                    4672024
model_v1::header_bench::_get_heights_by_prev_hash               5185377
model_v1::header_bench::_get_heights_by_hash                    5195615
model_v1::header_bench::_get_heights_by_timestamp               5276766
model_v1::maybevalue_bench::_filter                             6518055
model_v1::maybevalue_bench::_last                               6578947
model_v1::maybevalue_bench::_get                                6751739
model_v1::transaction_bench::_get_maybe                         6856829
model_v1::maybevalue_bench::_first                              6905600
model_v1::asset_bench::_exists                                 10157440
model_v1::maybevalue_bench::_exists                            12161012
model_v1::transaction_bench::_exists                           12221951
model_v1::utxo_bench::_exists                                  12671059
model_v1::input_bench::_exists                                 12703252
model_v1::block_bench::_exists                                 16767270
model_v1::header_bench::_exists                                16891892
```
<!-- END_BENCH -->

