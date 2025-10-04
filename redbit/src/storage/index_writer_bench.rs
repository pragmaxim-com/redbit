#[cfg(all(test, feature = "bench"))]
mod bench {
    use crate::storage::{index_test_utils, test_utils};
    use test::{black_box, Bencher};
    use crate::storage::test_utils::TxHash;

    pub(crate) fn tx_hash_dataset(cap: usize) -> Vec<TxHash> {
        let mut vals = Vec::with_capacity(cap);
        for i in 0..cap {
            // 3–4 bytes is enough to exercise sharding without dominating clone costs
            let v = test_utils::txh(&[(i as u8).wrapping_mul(17), (i as u8).wrapping_add(3), i as u8 ^ 0x5a]);
            vals.push(v);
        }
        vals
    }

    /// Baseline: single writer (non-sharded).
    fn bench_any_for_index_writer(m_values: usize, lru_cache: usize, b: &mut Bencher) {
        let name = format!("bench_idx_any_m{m_values}_c{lru_cache}");
        let (_owner_db, writer, _cache, _, _) = index_test_utils::setup_index_defs::<u32, TxHash>(&name, 1000);

        let tx_hashes = tx_hash_dataset(m_values);

        writer.begin().expect("begin");
        for (i, v) in tx_hashes.iter().cloned().enumerate() {
            writer.insert_kv(i as u32, v).expect("insert");
        }

        // warm steady-state path
        let _ = writer.get_any_for_index(tx_hashes.clone()).expect("warmup");

        b.iter(|| {
            let out = writer.get_any_for_index(tx_hashes.clone()).expect("bench call");
            black_box(out);
        });

        writer.flush().expect("flush");
        writer.shutdown().expect("shutdown");
    }

    /// ShardedWriter: shards × values × cache.
    fn bench_any_for_index_sharded(shards: usize, m_values: usize, lru_cache: usize, b: &mut Bencher) {
        assert!(shards >= 2);
        let prefix = format!("bench_idx_any_s{shards}_m{m_values}_c{lru_cache}");
        let (_owned, weak_dbs) = test_utils::mk_shard_dbs(shards, &prefix);
        let (s_writer_writer, _vp, _defs) = index_test_utils::mk_sharded_writer::<TxHash>(&prefix, shards, lru_cache, weak_dbs);

        let tx_hashes = tx_hash_dataset(m_values);

        s_writer_writer.begin().expect("begin");
        for (i, v) in tx_hashes.iter().cloned().enumerate() {
            s_writer_writer.insert_kv(i as u32, v).expect("insert");
        }

        // warm steady-state path
        let _ = s_writer_writer.get_any_for_index(tx_hashes.clone()).expect("warmup");

        b.iter(|| {
            let out = s_writer_writer.get_any_for_index(tx_hashes.clone()).expect("bench call");
            black_box(out);
        });

        s_writer_writer.flush().expect("flush");
        s_writer_writer.shutdown().expect("shutdown");
    }

    // -------------------- Writer (LRU: 0, 5000) --------------------
    #[bench] fn writer_m100_c0(b: &mut Bencher)   { bench_any_for_index_writer(100, 0, b); }
    #[bench] fn writer_m100_c5000(b: &mut Bencher){ bench_any_for_index_writer(100, 5000, b); }

    #[bench] fn writer_m1000_c0(b: &mut Bencher)   { bench_any_for_index_writer(1000, 0, b); }
    #[bench] fn writer_m1000_c5000(b: &mut Bencher){ bench_any_for_index_writer(1000, 5000, b); }

    #[bench] fn writer_m5000_c0(b: &mut Bencher)   { bench_any_for_index_writer(5000, 0, b); }
    #[bench] fn writer_m5000_c5000(b: &mut Bencher){ bench_any_for_index_writer(5000, 5000, b); }

    // -------------------- Sharded (shards: 2) --------------------
    #[bench] fn s_writer_s2_m100_c0(b: &mut Bencher)   { bench_any_for_index_sharded(2, 100, 0, b); }
    #[bench] fn s_writer_s2_m100_c5000(b: &mut Bencher){ bench_any_for_index_sharded(2, 100, 5000, b); }

    #[bench] fn s_writer_s2_m1000_c0(b: &mut Bencher)   { bench_any_for_index_sharded(2, 1000, 0, b); }
    #[bench] fn s_writer_s2_m1000_c5000(b: &mut Bencher){ bench_any_for_index_sharded(2, 1000, 5000, b); }

    #[bench] fn s_writer_s2_m5000_c0(b: &mut Bencher)   { bench_any_for_index_sharded(2, 5000, 0, b); }
    #[bench] fn s_writer_s2_m5000_c5000(b: &mut Bencher){ bench_any_for_index_sharded(2, 5000, 5000, b); }

    // -------------------- Sharded (shards: 4) --------------------
    #[bench] fn s_writer_s4_m100_c0(b: &mut Bencher)   { bench_any_for_index_sharded(4, 100, 0, b); }
    #[bench] fn s_writer_s4_m100_c5000(b: &mut Bencher){ bench_any_for_index_sharded(4, 100, 5000, b); }

    #[bench] fn s_writer_s4_m1000_c0(b: &mut Bencher)   { bench_any_for_index_sharded(4, 1000, 0, b); }
    #[bench] fn s_writer_s4_m1000_c5000(b: &mut Bencher){ bench_any_for_index_sharded(4, 1000, 5000, b); }

    #[bench] fn s_writer_s4_m5000_c0(b: &mut Bencher)   { bench_any_for_index_sharded(4, 5000, 0, b); }
    #[bench] fn s_writer_s4_m5000_c5000(b: &mut Bencher){ bench_any_for_index_sharded(4, 5000, 5000, b); }

    // -------------------- Sharded (shards: 8) --------------------
    #[bench] fn s_writer_s8_m100_c0(b: &mut Bencher)   { bench_any_for_index_sharded(8, 100, 0, b); }
    #[bench] fn s_writer_s8_m100_c5000(b: &mut Bencher){ bench_any_for_index_sharded(8, 100, 5000, b); }

    #[bench] fn s_writer_s8_m1000_c0(b: &mut Bencher)   { bench_any_for_index_sharded(8, 1000, 0, b); }
    #[bench] fn s_writer_s8_m1000_c5000(b: &mut Bencher){ bench_any_for_index_sharded(8, 1000, 5000, b); }

    #[bench] fn s_writer_s8_m5000_c0(b: &mut Bencher)   { bench_any_for_index_sharded(8, 5000, 0, b); }
    #[bench] fn s_writer_s8_m5000_c5000(b: &mut Bencher){ bench_any_for_index_sharded(8, 5000, 5000, b); }
}
