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
model_v1::block_bench::_store_many                                   95
model_v1::block_bench::_persist                                      98
model_v1::block_bench::_remove                                      114
model_v1::block_bench::_store                                       116
model_v1::transaction_bench::_persist                               117
model_v1::transaction_bench::_store_many                            119
model_v1::transaction_bench::_store                                 121
model_v1::transaction_bench::_remove                                122
model_v1::utxo_bench::_store                                        213
model_v1::utxo_bench::_store_many                                   222
model_v1::utxo_bench::_persist                                      234
model_v1::utxo_bench::_remove                                       238
model_v1::input_bench::_store_many                                  363
model_v1::input_bench::_store                                       365
model_v1::header_bench::_persist                                    876
model_v1::header_bench::_remove                                    1531
model_v1::asset_bench::_persist                                    1868
model_v1::input_bench::_persist                                    2258
model_v1::maybevalue_bench::_persist                               2377
model_v1::asset_bench::_remove                                     2441
model_v1::input_bench::_remove                                     2678
model_v1::maybevalue_bench::_remove                                3083
model_v1::header_bench::_store_many                                3779
model_v1::header_bench::_store                                     3966
model_v1::block_bench::_take                                       4402
model_v1::block_bench::_tail                                       4439
model_v1::block_bench::_stream_range                               6226
model_v1::asset_bench::_store_many                                 6719
model_v1::transaction_bench::_stream_blocks_by_hash                6720
model_v1::asset_bench::_store                                      7274
model_v1::transaction_bench::_stream_range                         8724
model_v1::block_bench::_get                                        9268
model_v1::block_bench::_get_transactions                           9273
model_v1::block_bench::_first                                      9297
model_v1::maybevalue_bench::_store_many                            9297
model_v1::block_bench::_last                                       9372
model_v1::transaction_bench::_stream_by_hash                       9594
model_v1::maybevalue_bench::_store                                 9977
model_v1::utxo_bench::_stream_transactions_by_address             10637
model_v1::transaction_bench::_stream_ids_by_hash                  11705
model_v1::transaction_bench::_tail                                14276
model_v1::transaction_bench::_take                                14298
model_v1::utxo_bench::_stream_range                               19006
model_v1::utxo_bench::_stream_by_address                          20504
model_v1::asset_bench::_stream_utxos_by_name                      21895
model_v1::utxo_bench::_stream_ids_by_address                      23441
model_v1::transaction_bench::_get_by_hash                         29884
model_v1::transaction_bench::_get                                 30088
model_v1::transaction_bench::_first                               30334
model_v1::transaction_bench::_last                                30670
model_v1::block_bench::_range                                     34196
model_v1::header_bench::_stream_range_by_duration                 34988
model_v1::header_bench::_stream_range_by_timestamp                35487
model_v1::header_bench::_stream_range                             36544
model_v1::block_bench::_filter                                    38636
model_v1::header_bench::_stream_by_duration                       39347
model_v1::transaction_bench::_range                               39544
model_v1::header_bench::_stream_by_prev_hash                      39592
model_v1::header_bench::_stream_by_hash                           39697
model_v1::header_bench::_stream_heights_by_hash                   39773
model_v1::header_bench::_stream_by_timestamp                      39849
model_v1::header_bench::_stream_heights_by_duration               39928
model_v1::header_bench::_stream_heights_by_prev_hash              41852
model_v1::header_bench::_stream_heights_by_timestamp              41958
model_v1::asset_bench::_stream_range                              59811
model_v1::transaction_bench::_get_utxos                           64167
model_v1::transaction_bench::_filter                              65851
model_v1::asset_bench::_stream_by_name                            69869
model_v1::asset_bench::_stream_ids_by_name                        73783
model_v1::utxo_bench::_tail                                       90525
model_v1::maybevalue_bench::_stream_range                         92743
model_v1::utxo_bench::_take                                       92834
model_v1::input_bench::_stream_range                              94367
model_v1::maybevalue_bench::_stream_by_hash                      122303
model_v1::maybevalue_bench::_stream_ids_by_hash                  126754
model_v1::utxo_bench::_range                                     147605
model_v1::utxo_bench::_get_by_address                            225505
model_v1::utxo_bench::_first                                     241857
model_v1::utxo_bench::_get                                       243322
model_v1::utxo_bench::_last                                      246105
model_v1::utxo_bench::_filter                                    260995
model_v1::utxo_bench::_get_assets                                277423
model_v1::asset_bench::_tail                                     308916
model_v1::asset_bench::_range                                    315533
model_v1::header_bench::_tail                                    321105
model_v1::header_bench::_range_by_duration                       327259
model_v1::transaction_bench::_get_inputs                         333016
model_v1::input_bench::_range                                    338707
model_v1::header_bench::_range                                   338709
model_v1::header_bench::_take                                    340333
model_v1::input_bench::_tail                                     346649
model_v1::header_bench::_range_by_timestamp                      349984
model_v1::asset_bench::_take                                     352473
model_v1::maybevalue_bench::_range                               356888
model_v1::maybevalue_bench::_tail                                365762
model_v1::input_bench::_take                                     392355
model_v1::maybevalue_bench::_take                                412213
model_v1::asset_bench::_get_by_name                             1558142
model_v1::header_bench::_get_by_duration                        1708205
model_v1::header_bench::_get_by_timestamp                       1872835
model_v1::header_bench::_get_by_prev_hash                       1904653
model_v1::header_bench::_get_by_hash                            1924298
model_v1::header_bench::_filter                                 2351503
model_v1::asset_bench::_filter                                  2394923
model_v1::asset_bench::_get                                     2600307
model_v1::header_bench::_last                                   2649498
model_v1::header_bench::_first                                  2670584
model_v1::asset_bench::_first                                   2687016
model_v1::block_bench::_get_header                              2723534
model_v1::asset_bench::_last                                    2787223
model_v1::header_bench::_get                                    2845193
model_v1::maybevalue_bench::_get_by_hash                        3114392
model_v1::asset_bench::_get_ids_by_name                         3467046
model_v1::utxo_bench::_get_ids_by_address                       3538821
model_v1::input_bench::_filter                                  4171359
model_v1::input_bench::_get                                     4342351
model_v1::header_bench::_get_heights_by_duration                4359578
model_v1::input_bench::_last                                    4418913
model_v1::input_bench::_first                                   4543389
model_v1::transaction_bench::_get_ids_by_hash                   4953192
model_v1::maybevalue_bench::_get_ids_by_hash                    5098919
model_v1::header_bench::_get_heights_by_prev_hash               5232589
model_v1::header_bench::_get_heights_by_hash                    5261496
model_v1::header_bench::_get_heights_by_timestamp               5537405
model_v1::maybevalue_bench::_last                               6679581
model_v1::maybevalue_bench::_first                              6877106
model_v1::maybevalue_bench::_filter                             6908463
model_v1::maybevalue_bench::_get                                6992518
model_v1::transaction_bench::_get_maybe                         7039775
model_v1::asset_bench::_exists                                  9485866
model_v1::input_bench::_exists                                 10997471
model_v1::utxo_bench::_exists                                  11128422
model_v1::maybevalue_bench::_exists                            12685526
model_v1::transaction_bench::_exists                           13386881
model_v1::header_bench::_exists                                17448962
model_v1::block_bench::_exists                                 17568517
```
<!-- END_BENCH -->

