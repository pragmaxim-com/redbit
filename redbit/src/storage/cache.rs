use crate::storage::init::{DbDef, DbDefWithCache};

/// Weighted proportional allocation using largest remainder (Hamilton),
/// operating purely in **MB**. Zero weights get 0 MB.
/// Deterministic tie-breaking by original order.
///
/// NOTE: `db_cache_in_mb` in the *result* is **per-shard**. We first allocate
/// per-column MB, then divide by `shards` (asserting shards >= 2).
pub fn allocate_cache_mb(db_defs: &[DbDef], total_mb: u64) -> Vec<DbDefWithCache> {
    if db_defs.is_empty() || total_mb == 0 {
        return db_defs.iter().map(|d| DbDefWithCache::new(d.clone(), 0)).collect();
    }

    let sum_w = sum_positive_weights(db_defs);
    if sum_w == 0 {
        return db_defs.iter().map(|d| DbDefWithCache::new(d.clone(), 0)).collect();
    }

    let mut shares = compute_shares_mb(db_defs, total_mb, sum_w);
    let base_sum_mb: u64 = shares.iter().map(|s| s.base_mb).sum();
    let remainder = total_mb.saturating_sub(base_sum_mb);
    if remainder > 0 {
        distribute_remainder_mb(&mut shares, remainder, db_defs);
    }

    collect_allocations_mb(&shares, db_defs)
}

fn sum_positive_weights(db_defs: &[DbDef]) -> u64 {
    db_defs.iter().map(|d| d.db_cache_weight_or_zero as u64).filter(|&w| w > 0).sum()
}

#[derive(Clone, Debug)]
struct Share { idx: usize, base_mb: u64, rem_num: u64 }

fn compute_shares_mb(db_defs: &[DbDef], total_mb: u64, sum_w: u64) -> Vec<Share> {
    let mut v = Vec::with_capacity(db_defs.len());
    for (idx, d) in db_defs.iter().enumerate() {
        let w = d.db_cache_weight_or_zero as u64;
        if w == 0 {
            v.push(Share { idx, base_mb: 0, rem_num: 0 });
            continue;
        }
        // ideal = total_mb * w / sum_w
        let prod = total_mb.saturating_mul(w);
        let base = prod / sum_w;
        let rem  = prod % sum_w;
        v.push(Share { idx, base_mb: base, rem_num: rem });
    }
    v
}

fn distribute_remainder_mb(shares: &mut [Share], mut remainder: u64, db_defs: &[DbDef]) {
    // Sort by (remainder desc, index asc) for deterministic ties
    shares.sort_by(|a, b| match b.rem_num.cmp(&a.rem_num) {
        core::cmp::Ordering::Equal => a.idx.cmp(&b.idx),
        other => other,
    });
    for s in shares.iter_mut() {
        if remainder == 0 { break; }
        if db_defs[s.idx].db_cache_weight_or_zero > 0 {
            s.base_mb = s.base_mb.saturating_add(1);
            remainder -= 1;
        }
    }
    // Restore original order
    shares.sort_by_key(|s| s.idx);
}

fn collect_allocations_mb(shares: &[Share], db_defs: &[DbDef]) -> Vec<DbDefWithCache> {
    shares.iter().map(|s| {
        let def = &db_defs[s.idx];
        let shards = def.shards;
        assert!(shards >= 1, "column `{}` must have at least 1 shard (got {shards})", def.name);

        let per_shard_mb = if shards == 1 { s.base_mb } else { s.base_mb / shards as u64 };

        DbDefWithCache {
            name: def.name.clone(),
            db_cache_weight: def.db_cache_weight_or_zero,
            db_cache_in_mb: cast_u64_to_usize(per_shard_mb),
            lru_cache: def.lru_cache_size_or_zero,
            shards,
        }
    }).collect()
}

fn cast_u64_to_usize(x: u64) -> usize {
    if x > (usize::MAX as u64) { usize::MAX } else { x as usize }
}

#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use super::*;

    // Build DbDefs with a fixed shard count (must be ≥ 2 now).
    fn defs(ws: &[usize], shards: usize) -> Vec<DbDef> {
        assert!(shards >= 1, "tests must use shards >= 1");
        ws.iter().enumerate().map(|(i, &w)| DbDef {
            name: format!("db{i}"),
            shards,
            db_cache_weight_or_zero: w,
            lru_cache_size_or_zero: 0,
        }).collect()
    }

    // Sum of per-shard MB across ALL shards (i.e., column total)
    fn sum_total_mb(v: &[DbDefWithCache]) -> u64 {
        v.iter()
            .map(|d| (d.db_cache_in_mb as u64) * (d.shards as u64))
            .sum()
    }

    // For convenience, but remember this is PER-SHARD, not column total.
    fn sum_per_shard_mb(v: &[DbDefWithCache]) -> u64 {
        v.iter().map(|d| d.db_cache_in_mb as u64).sum()
    }

    #[test]
    fn empty_or_zero_total_all_zero() {
        let out = allocate_cache_mb(&[], 42);
        assert!(out.is_empty());

        let out2 = allocate_cache_mb(&defs(&[1,2,3], 2), 0);
        assert_eq!(out2.len(), 3);
        assert!(out2.iter().all(|d| d.db_cache_in_mb == 0));
        assert_eq!(sum_total_mb(&out2), 0);
    }

    #[test]
    fn all_zero_weights_all_zero() {
        let out = allocate_cache_mb(&defs(&[0,0,0], 2), 10_000);
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|d| d.db_cache_in_mb == 0));
        assert_eq!(sum_total_mb(&out), 0);
    }

    #[test]
    fn proportional_split_exact_sum_mb() {
        // total 10 GiB → 10240 MB; weights 10 and 5 → 2:1
        // Column-level Hamilton gives 6827 and 3413 MB.
        // With shards=2, per-shard becomes floor(6827/2)=3413 and floor(3413/2)=1706.
        let total_mb = 10_u64 * 1024;
        let out = allocate_cache_mb(&defs(&[10, 5], 2), total_mb);
        assert_eq!(out.len(), 2);

        // Check per-SHARD expectations (deterministic):
        assert_eq!(out[0].db_cache_in_mb as u64, 3413);
        assert_eq!(out[1].db_cache_in_mb as u64, 1706);

        // Aggregated across shards we should be ≤ total (division drops remainders).
        let agg = sum_total_mb(&out);
        assert!(agg <= total_mb, "agg {} must be <= total {}", agg, total_mb);

        // Here the drop is exactly 2 MB (one remainder per column).
        assert_eq!(total_mb - agg, 2);
    }

    #[test]
    fn zero_weight_entries_get_zero_even_with_remainder() {
        // total 5 MB; weights: 0,1,0,1; shards=2
        // Active columns get 2 and 3 MB at the column level -> per-shard 1 and 1.
        let out = allocate_cache_mb(&defs(&[0,1,0,1], 2), 5);
        assert_eq!(out.len(), 4);

        assert_eq!(out[0].db_cache_in_mb, 0);
        assert_eq!(out[2].db_cache_in_mb, 0);

        // Both active columns should have per-shard = 1
        assert_eq!(out[1].db_cache_in_mb, 1);
        assert_eq!(out[3].db_cache_in_mb, 1);

        // Aggregated across shards: 1*2 + 1*2 = 4 <= 5 (1 MB lost due to division by shards)
        assert_eq!(sum_total_mb(&out), 4);
    }

    #[test]
    fn deterministic_ties_by_input_order() {
        // 3 equal weights, total 5 MB -> column-level bases 1 each, 2 remainder MB -> first two get them
        // Column totals: [2,2,1]
        // With shards=2, per-shard: [1,1,0]
        let out = allocate_cache_mb(&defs(&[1,1,1], 2), 5);
        let bytes: Vec<usize> = out.iter().map(|d| d.db_cache_in_mb).collect();
        assert_eq!(bytes, vec![1,1,0]);

        // Aggregated across shards: (1+1+0)*2 = 4, so we lose 1 MB on division.
        assert_eq!(sum_total_mb(&out), 4);
        assert_eq!(sum_per_shard_mb(&out), 2); // 1+1+0
    }

    #[test]
    fn many_items_small_total_single_mb_assigned() {
        // With shards=2 and total=1 MB, any column that gets 1 MB at column-level
        // will end up with per-shard 0. So all per-shard are 0 and aggregated total is 0.
        let n = 37;
        let out = allocate_cache_mb(
            &vec![DbDef { name: "x".into(), shards: 2, db_cache_weight_or_zero: 1, lru_cache_size_or_zero: 0 }; n],
            1
        );
        assert_eq!(out.len(), n);
        assert!(out.iter().all(|d| d.db_cache_in_mb == 0));
        assert_eq!(sum_total_mb(&out), 0);
    }

    #[test]
    fn compute_shares_mb_basic_invariants() {
        // This exercises the pre-division Hamilton stage; unchanged by shards.
        let defs = defs(&[2,3,5], 2);
        let total = 1_000u64; // MB
        let sum_w = sum_positive_weights(&defs);
        let shares = compute_shares_mb(&defs, total, sum_w);
        assert_eq!(shares.len(), 3);
        let base_sum: u64 = shares.iter().map(|s| s.base_mb).sum();
        assert!(base_sum <= total);
        assert!(shares.iter().all(|s| s.rem_num < sum_w));
    }

    #[test]
    fn distribute_remainder_mb_adds_exactly_r() {
        // Also pre-division; unchanged.
        let defs = defs(&[1,1,1,1], 2);
        let total = 6u64; // MB; base=1 each (4), remainder=2
        let sum_w = sum_positive_weights(&defs);
        let mut shares = compute_shares_mb(&defs, total, sum_w);
        let base_sum: u64 = shares.iter().map(|s| s.base_mb).sum();
        let r = total - base_sum;
        distribute_remainder_mb(&mut shares, r, &defs);

        shares.sort_by_key(|s| s.idx);
        let bases: Vec<u64> = shares.iter().map(|s| s.base_mb).collect();
        assert_eq!(bases, vec![2,2,1,1]);
        assert_eq!(bases.iter().sum::<u64>(), total);
    }

    #[test]
    fn cast_u64_to_usize_saturates_if_needed() {
        let big = u64::MAX;
        let casted = cast_u64_to_usize(big);
        if usize::BITS == 64 {
            assert_eq!(casted, usize::MAX);
        } else {
            assert_eq!(casted, usize::MAX);
        }
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod bench {
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