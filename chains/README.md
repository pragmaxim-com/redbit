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
model_v1::block_bench::_store_many                                   98
model_v1::block_bench::_persist                                     100
model_v1::transaction_bench::_store_many                            122
model_v1::block_bench::_store                                       123
model_v1::block_bench::_remove                                      125
model_v1::transaction_bench::_persist                               127
model_v1::transaction_bench::_store                                 127
model_v1::transaction_bench::_remove                                136
model_v1::utxo_bench::_store_many                                   241
model_v1::utxo_bench::_store                                        242
model_v1::utxo_bench::_persist                                      258
model_v1::utxo_bench::_remove                                       268
model_v1::input_bench::_store                                       371
model_v1::input_bench::_store_many                                  372
model_v1::header_bench::_persist                                    881
model_v1::header_bench::_remove                                    1475
model_v1::asset_bench::_persist                                    1914
model_v1::input_bench::_persist                                    2359
model_v1::asset_bench::_remove                                     2496
model_v1::maybevalue_bench::_persist                               2496
model_v1::input_bench::_remove                                     2677
model_v1::maybevalue_bench::_remove                                3025
model_v1::header_bench::_store_many                                3624
model_v1::header_bench::_store                                     3815
model_v1::block_bench::_take                                       4367
model_v1::block_bench::_tail                                       4393
model_v1::block_bench::_stream_range                               6198
model_v1::asset_bench::_store_many                                 6473
model_v1::transaction_bench::_stream_blocks_by_hash                6761
model_v1::asset_bench::_store                                      7046
model_v1::maybevalue_bench::_store_many                            8614
model_v1::block_bench::_first                                      8706
model_v1::block_bench::_get                                        8790
model_v1::transaction_bench::_stream_range                         8851
model_v1::block_bench::_get_transactions                           8883
model_v1::block_bench::_last                                       8890
model_v1::maybevalue_bench::_store                                 9212
model_v1::transaction_bench::_stream_by_hash                       9568
model_v1::utxo_bench::_stream_transactions_by_address             10970
model_v1::transaction_bench::_stream_ids_by_hash                  11818
model_v1::transaction_bench::_tail                                14042
model_v1::transaction_bench::_take                                14214
model_v1::utxo_bench::_stream_range                               20085
model_v1::utxo_bench::_stream_by_address                          21097
model_v1::asset_bench::_stream_utxos_by_name                      22066
model_v1::utxo_bench::_stream_ids_by_address                      23925
model_v1::transaction_bench::_get_by_hash                         29131
model_v1::transaction_bench::_get                                 29348
model_v1::transaction_bench::_first                               29664
model_v1::transaction_bench::_last                                29728
model_v1::block_bench::_range                                     33508
model_v1::header_bench::_stream_range_by_duration                 35331
model_v1::header_bench::_stream_range_by_timestamp                35740
model_v1::header_bench::_stream_range                             35882
model_v1::block_bench::_filter                                    36756
model_v1::transaction_bench::_range                               39061
model_v1::header_bench::_stream_by_prev_hash                      40701
model_v1::header_bench::_stream_by_duration                       40866
model_v1::header_bench::_stream_by_hash                           40914
model_v1::header_bench::_stream_by_timestamp                      40974
model_v1::header_bench::_stream_heights_by_timestamp              41419
model_v1::header_bench::_stream_heights_by_prev_hash              42207
model_v1::header_bench::_stream_heights_by_duration               42335
model_v1::header_bench::_stream_heights_by_hash                   42380
model_v1::asset_bench::_stream_range                              59137
model_v1::transaction_bench::_get_utxos                           62168
model_v1::transaction_bench::_filter                              63769
model_v1::asset_bench::_stream_by_name                            69691
model_v1::asset_bench::_stream_ids_by_name                        73104
model_v1::utxo_bench::_tail                                       87977
model_v1::utxo_bench::_take                                       91963
model_v1::input_bench::_stream_range                              93539
model_v1::maybevalue_bench::_stream_range                         94186
model_v1::maybevalue_bench::_stream_by_hash                      124780
model_v1::maybevalue_bench::_stream_ids_by_hash                  128027
model_v1::utxo_bench::_range                                     138426
model_v1::utxo_bench::_get_by_address                            214034
model_v1::utxo_bench::_get                                       231027
model_v1::utxo_bench::_first                                     232580
model_v1::utxo_bench::_last                                      235082
model_v1::utxo_bench::_filter                                    249413
model_v1::utxo_bench::_get_assets                                267131
model_v1::asset_bench::_range                                    295770
model_v1::asset_bench::_tail                                     300245
model_v1::header_bench::_range_by_duration                       323348
model_v1::header_bench::_range                                   329499
model_v1::transaction_bench::_get_inputs                         329722
model_v1::header_bench::_tail                                    338935
model_v1::input_bench::_range                                    339688
model_v1::asset_bench::_take                                     343114
model_v1::header_bench::_range_by_timestamp                      344263
model_v1::header_bench::_take                                    353781
model_v1::input_bench::_tail                                     355380
model_v1::maybevalue_bench::_range                               364570
model_v1::maybevalue_bench::_tail                                365437
model_v1::input_bench::_take                                     396300
model_v1::maybevalue_bench::_take                                407174
model_v1::asset_bench::_get_by_name                             1435565
model_v1::header_bench::_get_by_duration                        1739493
model_v1::header_bench::_get_by_prev_hash                       1875715
model_v1::header_bench::_get_by_hash                            1875821
model_v1::header_bench::_get_by_timestamp                       1926077
model_v1::asset_bench::_filter                                  1988625
model_v1::asset_bench::_get                                     2198044
model_v1::header_bench::_filter                                 2316638
model_v1::asset_bench::_first                                   2442838
model_v1::asset_bench::_last                                    2483053
model_v1::block_bench::_get_header                              2536333
model_v1::header_bench::_get                                    2619653
model_v1::header_bench::_first                                  2682763
model_v1::header_bench::_last                                   2696508
model_v1::asset_bench::_get_ids_by_name                         3257329
model_v1::maybevalue_bench::_get_by_hash                        3452919
model_v1::utxo_bench::_get_ids_by_address                       3669725
model_v1::input_bench::_filter                                  3753895
model_v1::input_bench::_get                                     3935768
model_v1::header_bench::_get_heights_by_duration                4260940
model_v1::input_bench::_last                                    4566002
model_v1::input_bench::_first                                   4567253
model_v1::transaction_bench::_get_ids_by_hash                   4748789
model_v1::header_bench::_get_heights_by_hash                    4933643
model_v1::header_bench::_get_heights_by_prev_hash               4994257
model_v1::maybevalue_bench::_get_ids_by_hash                    5105166
model_v1::header_bench::_get_heights_by_timestamp               5568239
model_v1::maybevalue_bench::_filter                             6280224
model_v1::transaction_bench::_get_maybe                         6515082
model_v1::maybevalue_bench::_get                                6554798
model_v1::maybevalue_bench::_last                               6607202
model_v1::maybevalue_bench::_first                              6616382
model_v1::asset_bench::_exists                                  9349289
model_v1::input_bench::_exists                                 11648224
model_v1::utxo_bench::_exists                                  12070006
model_v1::transaction_bench::_exists                           12235409
model_v1::maybevalue_bench::_exists                            12359412
model_v1::block_bench::_exists                                 16758840
model_v1::header_bench::_exists                                16832183
```
<!-- END_BENCH -->

