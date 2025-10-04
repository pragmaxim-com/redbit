#[cfg(all(test, not(feature = "integration")))]
mod entity_tests {
    use std::num::NonZeroUsize;
    use std::time::Instant;
    use lru::LruCache;

    #[inline(always)]
    fn splitmix64(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    #[inline(always)]
    fn make_txhash_bytes(i: u64) -> [u8; 32] {
        let mut out = [0u8; 32];
        let mut x = splitmix64(i ^ 0xDEAD_BEEF_CAFE_BABE);
        for chunk in out.chunks_mut(8) {
            x = splitmix64(x);
            chunk.copy_from_slice(&x.to_le_bytes());
        }
        out
    }

    pub fn txhash_lru_insert_then_read(n: u64) {
        assert!(n <= usize::MAX as u64, "n too large for usize");
        let cap = NonZeroUsize::new(n as usize).expect("n must be > 0");

        let mut lru: LruCache<[u8; 32], u8> = LruCache::new(cap);

        let t0 = Instant::now();
        for i in 0..n {
            let k = make_txhash_bytes(i);
            lru.put(k, 0xFF);

            if i % 1_000_000 == 0 {
                println!("inserted {:>9} / {}", i, n);
            }
        }
        let t_insert = t0.elapsed();

        assert_eq!(lru.len(), n as usize);

        let t1 = Instant::now();
        let mut hits: u64 = 0;
        for i in 0..n {
            let k = make_txhash_bytes(i);
            if lru.get(&k).is_some() {
                hits += 1;
            } else {
                panic!("unexpected miss at i={}", i);
            }
            if i % 1_000_000 == 0 {
                println!("read     {:>9} / {}", i, n);
            }
        }
        let t_read = t1.elapsed();

        println!("Done. entries={}, hits={}, insert_time={:?}, read_time={:?}", lru.len(), hits, t_insert, t_read);
    }

    #[test]
    fn tiny_sanity_txhash_lru() {
        txhash_lru_insert_then_read(1_000);
    }
}