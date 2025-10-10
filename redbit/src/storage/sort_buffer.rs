use crate::CopyOwnedValue;
use redb::Key;
use std::borrow::Borrow;
use std::cmp::Ordering;

/// Keeps at most one sorted run at each level; on push it “carries” upward by merging.
/// Total moves per element: O(log N). Flush typically deals with <= 13 runs for 25k items.
pub struct MergeBuffer<K, V> {
    levels: Vec<Option<Vec<(K, V)>>>,
}

impl<K, V> Default for MergeBuffer<K, V> {
    fn default() -> Self { Self { levels: Vec::new() } }
}

impl<K, V> MergeBuffer<K, V>
where
    K: CopyOwnedValue + Borrow<K::SelfType<'static>> + 'static,
    V: Key + Borrow<V::SelfType<'static>> + 'static,
{
    pub fn new() -> Self { Self { levels: Vec::new() } }

    /// Drop all runs without merging.
    pub fn clear(&mut self) {
        self.levels.clear(); // drops inner Vecs; O(total_len)
    }

    /// Is the buffer currently empty?
    pub fn is_empty(&self) -> bool {
        self.levels.iter().all(|s| s.is_none())
    }
    /// Construct from an **already sorted** run. No work needed on drain.
    pub fn from_sorted(sorted: Vec<(K, V)>) -> Self {
        if sorted.is_empty() {
            return Self::new();
        }
        MergeBuffer { levels: vec![Some(sorted)] }
    }

    pub fn push_unsorted(&mut self, mut run: Vec<(K, V)>) {
        if run.is_empty() { return; }
        run.sort_by(|(a,_),(b,_)| {
            let ab = K::as_bytes(a.borrow());
            let bb = K::as_bytes(b.borrow());
            K::compare(ab.as_ref(), bb.as_ref())
        });
        self.push_sorted(run);
    }

    pub fn push_sorted(&mut self, mut run: Vec<(K, V)>) {
        if run.is_empty() { return; }
        let mut lvl = 0usize;
        loop {
            if self.levels.len() <= lvl {
                self.levels.resize_with(lvl + 1, || None);
            }
            if let Some(existing) = self.levels[lvl].take() {
                run = merge_sorted_by_key(existing, run); // stable
                lvl += 1;
            } else {
                self.levels[lvl] = Some(run);
                break;
            }
        }
    }

    pub fn take_sorted(&mut self) -> Vec<(K, V)> {
        let levels = std::mem::take(&mut self.levels);
        let mut acc: Option<Vec<(K, V)>> = None;
        for slot in levels.into_iter().flatten() {
            acc = Some(match acc {
                None => slot,
                Some(prev) => merge_sorted_by_key(prev, slot),
            });
        }
        acc.unwrap_or_default()
    }

    pub fn runs(&self) -> usize {
        self.levels.iter().filter(|s| s.is_some()).count()
    }
}

/// Stable merge of two key-sorted vectors.
pub fn merge_sorted_by_key<K, V>(left: Vec<(K, V)>, right: Vec<(K, V)>) -> Vec<(K, V)>
where
    K: CopyOwnedValue + Borrow<K::SelfType<'static>> + 'static,
    V: Key + Borrow<V::SelfType<'static>> + 'static,
{
    use std::iter::Peekable;

    let mut a: Peekable<std::vec::IntoIter<(K, V)>> = left.into_iter().peekable();
    let mut b: Peekable<std::vec::IntoIter<(K, V)>> = right.into_iter().peekable();
    let mut out: Vec<(K, V)> = Vec::with_capacity(a.size_hint().0 + b.size_hint().0);

    loop {
        match (a.peek(), b.peek()) {
            (Some((ka, _)), Some((kb, _))) => {
                let ord = {
                    let ab = K::as_bytes(ka.borrow());
                    let bb = K::as_bytes(kb.borrow());
                    K::compare(ab.as_ref(), bb.as_ref())
                };
                // Stable: take from `a` on Equal.
                if matches!(ord, Ordering::Less | Ordering::Equal) {
                    out.push(a.next().unwrap());
                } else {
                    out.push(b.next().unwrap());
                }
            }
            (Some(_), None) => { out.extend(a); break; }
            (None, Some(_)) => { out.extend(b); break; }
            (None, None) => break,
        }
    }
    out
}
#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use super::*;
    use crate::storage::test_utils::{addr, Address};

    // --- helpers (kill duplication) ---

    #[inline]
    fn sort_u32_addr_by_key(v: &mut Vec<(u32, Address)>) {
        use redb::{Key as RedbKey, Value as RedbValue};
        v.sort_by(|(x,_),(y,_)| {
            let xb = <u32 as RedbValue>::as_bytes(x);
            let yb = <u32 as RedbValue>::as_bytes(y);
            <u32 as RedbKey>::compare(xb.as_ref(), yb.as_ref())
        });
    }

    #[inline]
    fn assert_sorted_u32_addr(v: &[(u32, Address)]) {
        use redb::{Key as RedbKey, Value as RedbValue};
        use std::cmp::Ordering;
        for w in v.windows(2) {
            let (a, _) = &w[0];
            let (b, _) = &w[1];
            let ab = <u32 as RedbValue>::as_bytes(a);
            let bb = <u32 as RedbValue>::as_bytes(b);
            let ord = <u32 as RedbKey>::compare(ab.as_ref(), bb.as_ref());
            assert!(matches!(ord, Ordering::Less | Ordering::Equal),
                    "not sorted: {:?} then {:?}", a, b);
        }
    }

    // --- merge_sorted_by_key coverage ---

    #[test]
    fn runs_flush_produces_globally_sorted_vec() {
        let mut runs: Vec<Vec<(u32, Address)>> = Vec::new();

        let mut a = vec![(5, addr(&[1])), (2, addr(&[1])), (7, addr(&[1]))];
        let mut b = vec![(3, addr(&[2])), (8, addr(&[2])), (1, addr(&[2]))];
        let mut c = vec![(6, addr(&[3])), (10, addr(&[3])), (4, addr(&[3]))];

        sort_u32_addr_by_key(&mut a);
        sort_u32_addr_by_key(&mut b);
        sort_u32_addr_by_key(&mut c);
        runs.push(a); runs.push(b); runs.push(c);

        use std::collections::VecDeque;
        let mut q: VecDeque<Vec<(u32, Address)>> = runs.into();
        while q.len() > 1 {
            let lhs = q.pop_front().unwrap();
            let rhs = q.pop_front().unwrap();
            q.push_back(merge_sorted_by_key(lhs, rhs));
        }
        let out = q.pop_front().unwrap();
        assert_sorted_u32_addr(&out);
        let keys: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(keys, vec![1,2,3,4,5,6,7,8,10]);
    }

    // --- MergeBuffer tests ---

    #[test]
    fn merge_buffer_push_unsorted_many_chunks_then_take_sorted() {
        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();

        // push three tiny unsorted chunks
        mb.push_unsorted(vec![(5, addr(&[1])), (2, addr(&[1])), (7, addr(&[1]))]);
        assert_eq!(mb.runs(), 1, "after first chunk, exactly one run exists");

        mb.push_unsorted(vec![(3, addr(&[2])), (8, addr(&[2])), (1, addr(&[2]))]);
        assert_eq!(mb.runs(), 1, "two same-size chunks should carry-merge to one run");

        mb.push_unsorted(vec![(6, addr(&[3])), (10, addr(&[3])), (4, addr(&[3]))]);
        // now we expect two runs: one big (size 6) and one small (size 3)
        assert_eq!(mb.runs(), 2, "third chunk should sit at lower level alongside merged run");

        // result must be globally sorted and buffer drained
        let out = mb.take_sorted();
        assert_sorted_u32_addr(&out);
        let keys: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(keys, vec![1,2,3,4,5,6,7,8,10]);
        assert_eq!(mb.runs(), 0, "buffer should be drained after take_sorted()");
    }

    #[test]
    fn merge_buffer_push_sorted_carry_levels_collapse() {
        // three already-sorted runs (sizes equal → predictable carries)
        let mut r1 = vec![(2, addr(&[1])), (5, addr(&[1]))];
        let mut r2 = vec![(3, addr(&[2])), (7, addr(&[2]))];
        let mut r3 = vec![(1, addr(&[3])), (9, addr(&[3]))];
        sort_u32_addr_by_key(&mut r1);
        sort_u32_addr_by_key(&mut r2);
        sort_u32_addr_by_key(&mut r3);

        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();

        mb.push_sorted(r1);
        assert_eq!(mb.runs(), 1, "one sorted run → one level occupied");

        mb.push_sorted(r2);
        assert_eq!(mb.runs(), 1, "two level-0 runs must carry-merge into one higher run");

        mb.push_sorted(r3);
        assert_eq!(mb.runs(), 2, "third run occupies level-0 next to the merged higher run");

        let out = mb.take_sorted();
        assert_sorted_u32_addr(&out);
        assert_eq!(mb.runs(), 0, "drained after take_sorted()");
        let keys: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(keys, vec![1,2,3,5,7,9]);
    }

    #[test]
    fn merge_buffer_from_sorted_roundtrip_no_overhead() {
        // prepare a sorted run
        let mut run = vec![(4, addr(&[1])), (1, addr(&[1])), (3, addr(&[1])), (2, addr(&[1]))];
        sort_u32_addr_by_key(&mut run);

        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::from_sorted(run.clone());
        assert_eq!(mb.runs(), 1, "from_sorted should create exactly one run");

        let out = mb.take_sorted();
        assert_sorted_u32_addr(&out);
        assert_eq!(out.len(), run.len());
        let ks_out: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        let ks_ref: Vec<u32> = run.iter().map(|(k,_)| *k).collect();
        assert_eq!(ks_out, ks_ref);
        assert_eq!(mb.runs(), 0, "buffer drains on take_sorted()");
    }

    #[test]
    fn merge_buffer_stability_equal_keys_existing_before_new() {
        // two runs with the same key=5, different payloads; first run must come first
        let a1 = addr(&[0x01]); let a2 = addr(&[0x02]); let a3 = addr(&[0x03]);
        let mut r_old = vec![(5, a1.clone()), (5, a2.clone())];
        let mut r_new = vec![(5, a3.clone())];
        sort_u32_addr_by_key(&mut r_old);
        sort_u32_addr_by_key(&mut r_new);

        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();
        mb.push_sorted(r_old);
        assert_eq!(mb.runs(), 1);
        mb.push_sorted(r_new);
        assert_eq!(mb.runs(), 1, "equal-key new run should merge into existing run");

        let out = mb.take_sorted();
        let vals_5: Vec<Vec<u8>> = out.iter()
            .filter(|(k,_)| *k == 5)
            .map(|(_,a)| a.0.clone())
            .collect();
        assert_eq!(vals_5, vec![a1.0, a2.0, a3.0],
                   "stable merge: earlier run entries with equal keys must precede later ones");
        assert_eq!(mb.runs(), 0);
    }
}


#[cfg(all(test, feature = "bench"))]
mod bench {
    extern crate test;
    use crate::impl_copy_owned_value_identity;
    use crate::storage::async_boundary::CopyOwnedValue;
    use crate::storage::sort_buffer::MergeBuffer;
    use crate::storage::test_utils::TxHash;
    use test::Bencher;

    impl_copy_owned_value_identity!(TxHash);

    // xorshift64 PRNG (deterministic)
    #[inline]
    fn xorshift64(x: &mut u64) -> u64 {
        let mut v = *x;
        v ^= v << 13;
        v ^= v >> 7;
        v ^= v << 17;
        *x = v;
        v
    }

    #[inline]
    fn hash_from_seed(mut s: u64) -> TxHash {
        let mut bytes = [0u8; 32];
        for i in 0..32 {
            s = xorshift64(&mut s);
            bytes[i] = (s >> ((i & 7) * 8)) as u8;
        }
        TxHash(bytes)
    }

    // Build one tiny unsorted batch of length `len`
    fn build_unsorted(len: usize, seed: u64) -> Vec<(TxHash, u32)> {
        let mut s = seed ^ 0xCAFEBABE_DEADBEEFu64;
        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            let key = hash_from_seed(s);
            let val = (xorshift64(&mut s) as u32).rotate_left(7) ^ 0xA5A5_5A5A;
            v.push((key, val));
        }
        v
    }

    // Prebuild 1000 tiny batches (len=5), unsorted, outside the timer
    fn build_batches(n_batches: usize, len: usize, seed: u64) -> Vec<Vec<(TxHash, u32)>> {
        (0..n_batches)
            .map(|i| build_unsorted(len, seed.wrapping_add((i as u64 + 1) * 0x9E37_79B97F4A_7C15)))
            .collect()
    }

    fn merge_buffer_from_sorted_roundtrip() {
        use redb::{Key as RedbKey, Value as RedbValue};
        use std::cmp::Ordering;

        // prepare a sorted run
        let mut run = build_unsorted(32, 42);
        run.sort_by(|(a,_),(b,_)| {
            let ab = <TxHash as RedbValue>::as_bytes(a);
            let bb = <TxHash as RedbValue>::as_bytes(b);
            <TxHash as RedbKey>::compare(ab.as_ref(), bb.as_ref())
        });

        let mut mbuf = MergeBuffer::from_sorted(run.clone());
        assert_eq!(mbuf.runs(), 1);

        let out = mbuf.take_sorted();
        assert_eq!(out.len(), run.len());
        // verify equality & ordering
        for w in out.windows(2) {
            let (ka, _) = &w[0];
            let (kb, _) = &w[1];
            let ab = <TxHash as RedbValue>::as_bytes(ka);
            let bb = <TxHash as RedbValue>::as_bytes(kb);
            let ord = <TxHash as RedbKey>::compare(ab.as_ref(), bb.as_ref());
            assert!(matches!(ord, Ordering::Less | Ordering::Equal));
        }
    }

    #[bench]
    fn bench_merge_buffer_1000x5_txhash(b: &mut Bencher) {
        merge_buffer_from_sorted_roundtrip();
        let batches = build_batches(1_000, 5, 0xD00D_F00D_1234_5678);

        b.iter(|| {
            let mut mbuf: MergeBuffer<TxHash, u32> = MergeBuffer::new();
            for batch in batches.iter() {
                mbuf.push_unsorted(batch.clone());
            }
            let sorted = mbuf.take_sorted(); // minimal overhead if carries collapsed
            test::black_box(&sorted);
        });
    }

}
