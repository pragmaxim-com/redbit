use crate::{DbDef, DbDefWithCache};

/// Weighted proportional allocation using largest remainder (Hamilton),
/// operating purely in **MB**. Zero weights get 0 MB.
/// Deterministic tie-breaking by original order.
pub fn allocate_cache_mb(db_defs: &[DbDef], total_mb: u64) -> Vec<DbDefWithCache> {
    if db_defs.is_empty() || total_mb == 0 {
        return db_defs.iter().map(|d| DbDefWithCache {
            name: d.name.clone(),
            db_cache_weight: d.db_cache_weight_or_zero,
            lru_cache: d.lru_cache_size_or_zero,
            db_cache_in_mb: 0,
        }).collect();
    }

    let sum_w = sum_positive_weights(db_defs);
    if sum_w == 0 {
        return db_defs.iter().map(|d| DbDefWithCache {
            name: d.name.clone(),
            db_cache_weight: d.db_cache_weight_or_zero,
            lru_cache: d.lru_cache_size_or_zero,
            db_cache_in_mb: 0,
        }).collect();
    }

    let mut shares = compute_shares_mb(db_defs, total_mb, sum_w);
    let base_sum_mb: u64 = shares.iter().map(|s| s.base_mb).sum();
    let remainder = total_mb.saturating_sub(base_sum_mb);
    if remainder > 0 {
        distribute_remainder_mb(&mut shares, remainder, db_defs);
    }

    collect_allocations_mb(&shares, db_defs)
}

/* ========= helpers ========= */

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
    shares.iter().map(|s| DbDefWithCache {
        name: db_defs[s.idx].name.clone(),
        db_cache_weight: db_defs[s.idx].db_cache_weight_or_zero,
        db_cache_in_mb: cast_u64_to_usize(s.base_mb),
        lru_cache: db_defs[s.idx].lru_cache_size_or_zero
    }).collect()
}

#[inline]
fn cast_u64_to_usize(x: u64) -> usize {
    if x > (usize::MAX as u64) { usize::MAX } else { x as usize }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    fn defs(ws: &[usize]) -> Vec<DbDef> {
        ws.iter().enumerate().map(|(i, &w)| DbDef { name: format!("db{i}"), db_cache_weight_or_zero: w, lru_cache_size_or_zero: 0 }).collect()
    }

    fn sum_mb(v: &[DbDefWithCache]) -> u64 {
        v.iter().map(|d| d.db_cache_in_mb as u64).sum()
    }

    #[test]
    fn empty_or_zero_total_all_zero() {
        let out = allocate_cache_mb(&[], 42);
        assert!(out.is_empty());

        let out2 = allocate_cache_mb(&defs(&[1,2,3]), 0);
        assert_eq!(out2.len(), 3);
        assert!(out2.iter().all(|d| d.db_cache_in_mb == 0));
    }

    #[test]
    fn all_zero_weights_all_zero() {
        let out = allocate_cache_mb(&defs(&[0,0,0]), 10_000);
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|d| d.db_cache_in_mb == 0));
        assert_eq!(sum_mb(&out), 0);
    }

    #[test]
    fn proportional_split_exact_sum_mb() {
        // total 10 GiB → 10240 MB; weights 10 and 5 → 2:1
        let total_mb = 10_u64 * 1024;
        let out = allocate_cache_mb(&defs(&[10, 5]), total_mb);
        assert_eq!(out.len(), 2);

        // Ideal: 6826.6.. and 3413.3..? No—since it's MB, it's exactly 10240 * (10/15) = 6826.(6)
        // Largest remainder awards the extra MBs to the bigger remainder: 6827 and 3413 MB
        assert_eq!(sum_mb(&out), total_mb);
        assert_eq!(out[0].db_cache_in_mb as u64, 6827);
        assert_eq!(out[1].db_cache_in_mb as u64, 3413);
    }

    #[test]
    fn zero_weight_entries_get_zero_even_with_remainder() {
        // total 5 MB; weights: 0,1,0,1 -> active share: 2 items
        let out = allocate_cache_mb(&defs(&[0,1,0,1]), 5);
        assert_eq!(out[0].db_cache_in_mb, 0);
        assert_eq!(out[2].db_cache_in_mb, 0);
        assert_eq!(out[1].db_cache_in_mb + out[3].db_cache_in_mb, 5);
    }

    #[test]
    fn deterministic_ties_by_input_order() {
        // 3 equal weights, total 5 MB -> bases 1 each, 2 remainder MB -> first two get them
        let out = allocate_cache_mb(&defs(&[1,1,1]), 5);
        let bytes: Vec<usize> = out.into_iter().map(|d| d.db_cache_in_mb).collect();
        assert_eq!(bytes, vec![2,2,1]);
    }

    #[test]
    fn many_items_small_total_single_mb_assigned() {
        let n = 37;
        let out = allocate_cache_mb(&vec![DbDef { name: "x".into(), db_cache_weight_or_zero: 1, lru_cache_size_or_zero: 0 }; n], 1);
        assert_eq!(out.len(), n);
        let ones = out.iter().filter(|d| d.db_cache_in_mb == 1).count();
        let zeros = out.iter().filter(|d| d.db_cache_in_mb == 0).count();
        assert_eq!(ones, 1);
        assert_eq!(zeros, n - 1);
        assert_eq!(sum_mb(&out), 1);
    }

    #[test]
    fn compute_shares_mb_basic_invariants() {
        let defs = defs(&[2,3,5]);
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
        let defs = defs(&[1,1,1,1]);
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
        let casted = super::cast_u64_to_usize(big);
        if (usize::BITS as u32) == 64 {
            assert_eq!(casted, usize::MAX);
        } else {
            assert_eq!(casted, usize::MAX);
        }
    }
}
