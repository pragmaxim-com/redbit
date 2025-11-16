use crate::{DbKey, DbVal};
use redb::Key;
use std::borrow::Borrow;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// Keeps at most one sorted run at each level; on push it “carries” upward by merging.
/// Total moves per element: O(log N). Flush typically deals with <= 13 runs for 25k items.
#[derive(Clone)]
pub struct MergeBuffer<K, V> {
    levels: Vec<Option<Vec<(K, V)>>>,
}

impl<K, V> Default for MergeBuffer<K, V> {
    fn default() -> Self { Self { levels: Vec::new() } }
}

impl<K, V> MergeBuffer<K, V>
where
    K: DbKey,
    V: DbVal
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

    pub fn merge_unsorted(&mut self, mut run: Vec<(K, V)>) {
        if run.is_empty() { return; }
        run.sort_by(|(a,_),(b,_)| K::compare(K::as_bytes(a.borrow()).as_ref(), K::as_bytes(b.borrow()).as_ref()));
        self.merge_sorted(run);
    }

    pub fn append_sorted(&mut self, run: Vec<(K, V)>) {
        use std::cmp::Ordering;
        if run.is_empty() { return; }

        #[inline]
        fn last_key<K>(r: &[(K, impl Sized)]) -> &K { &r[r.len() - 1].0 }

        // Scan once: count runs, record the only-run index (if any), and the index of the run
        // whose last key is the global maximum (to enable O(m) append even with many runs).
        let mut n_runs = 0usize;
        let mut only_idx: Option<usize> = None;
        let mut max_idx: Option<usize>  = None;

        for (i, slot) in self.levels.iter().enumerate() {
            if slot.is_some() {
                n_runs += 1;
                if only_idx.is_none() { only_idx = Some(i); }
                max_idx = Some(match max_idx {
                    None => i,
                    Some(j) => {
                        let take_i = {
                            let ri = self.levels[i].as_ref().unwrap();
                            let rj = self.levels[j].as_ref().unwrap();
                            let li_b = K::as_bytes(last_key::<K>(ri).borrow());
                            let lj_b = K::as_bytes(last_key::<K>(rj).borrow());
                            matches!(K::compare(li_b.as_ref(), lj_b.as_ref()), Ordering::Greater)
                        };
                        if take_i { i } else { j }
                    }
                });
            }
        }

        // A) empty buffer → install directly
        if n_runs == 0 {
            self.levels.clear();
            self.levels.push(Some(run));
            return;
        }

        let new_first_key = &run[0].0;

        // B) exactly one run present
        if n_runs == 1 {
            let idx = only_idx.unwrap();

            // boundary check in an immutable scope
            let can_append = {
                let existing = self.levels[idx].as_ref().unwrap();
                let ex_last_b = K::as_bytes(last_key::<K>(existing).borrow());
                let nf_b      = K::as_bytes(new_first_key.borrow());
                !matches!(K::compare(ex_last_b.as_ref(), nf_b.as_ref()), Ordering::Greater)
            };

            if can_append {
                // Fast append: avoid realloc spikes across many small appends.
                let ex = self.levels[idx].as_mut().unwrap();
                ex.reserve(run.len());
                ex.extend(run);
                return;
            } else {
                // ❗ key change: DO NOT eagerly merge the whole large run.
                // Queue it as a new sorted run and let the binary-counter carry amortize.
                self.merge_sorted(run);
                return;
            }
        }

        // C) multiple runs present
        if let Some(mx) = max_idx {
            let can_append_to_max = {
                let ex = self.levels[mx].as_ref().unwrap();
                let ex_last_b = K::as_bytes(last_key::<K>(ex).borrow());
                let nf_b      = K::as_bytes(new_first_key.borrow());
                !matches!(K::compare(ex_last_b.as_ref(), nf_b.as_ref()), Ordering::Greater)
            };
            if can_append_to_max {
                let ex = self.levels[mx].as_mut().unwrap();
                ex.reserve(run.len());
                ex.extend(run);
                return;
            }
        }

        // general overlapping case → preserve amortized O(N log N)
        self.merge_sorted(run);
    }


    pub fn merge_sorted(&mut self, mut run: Vec<(K, V)>) {
        use std::cmp::Ordering;
        if run.is_empty() { return; }

        #[inline]
        fn first_key<K>(r: &[(K, impl Sized)]) -> &K { &r[0].0 }
        #[inline]
        fn last_key<K>(r: &[(K, impl Sized)]) -> &K { &r[r.len() - 1].0 }

        let mut lvl = 0usize;
        loop {
            if self.levels.len() == lvl {
                self.levels.push(Some(run));
                break;
            }
            match self.levels[lvl].take() {
                None => { self.levels[lvl] = Some(run); break; }
                Some(mut existing) => {
                    // append / prepend fast-paths
                    let can_append = {
                        let ex_last_b = K::as_bytes(last_key::<K>(&existing).borrow());
                        let rn_first_b = K::as_bytes(first_key::<K>(&run).borrow());
                        matches!(K::compare(ex_last_b.as_ref(), rn_first_b.as_ref()), Ordering::Less | Ordering::Equal)
                    };
                    if can_append {
                        existing.reserve_exact(run.len());
                        existing.extend(run);
                        self.levels[lvl] = Some(existing);
                        break;
                    }
                    let can_prepend = {
                        let rn_last_b = K::as_bytes(last_key::<K>(&run).borrow());
                        let ex_first_b = K::as_bytes(first_key::<K>(&existing).borrow());
                        matches!(K::compare(rn_last_b.as_ref(), ex_first_b.as_ref()), Ordering::Less)
                    };
                    if can_prepend {
                        let mut front = run;
                        front.reserve_exact(existing.len());
                        front.extend(existing);
                        self.levels[lvl] = Some(front);
                        break;
                    }

                    // Overlap → galloping merge + carry
                    run = merge_sorted_by_key_gallop(existing, run);
                    lvl += 1;
                }
            }
        }
    }

    pub fn take_sorted(&mut self) -> Vec<(K, V)> {

        #[inline]
        fn first_key<K>(r: &[(K, impl Sized)]) -> &K { &r[0].0 }
        #[inline]
        fn last_key<K>(r: &[(K, impl Sized)]) -> &K { &r[r.len() - 1].0 }

        // 1) Collect existing runs.
        let mut runs: Vec<Vec<(K, V)>> = {
            let levels = std::mem::take(&mut self.levels);
            levels.into_iter().flatten().collect()
        };

        match runs.len() {
            0 => return Vec::new(),
            1 => return runs.pop().unwrap(),
            _ => {}
        }

        // 2) Sort runs by first key, so we can try concatenation fast-path.
        runs.sort_by(|a, b| {
            let ab = K::as_bytes(first_key::<K>(a).borrow());
            let bb = K::as_bytes(first_key::<K>(b).borrow());
            K::compare(ab.as_ref(), bb.as_ref())
        });

        // 3) Concatenation fast-path: if every adjacent pair is disjoint/adjacent (last ≤ next.first)
        let disjoint_chain = runs.windows(2).all(|w| {
            let la = K::as_bytes(last_key::<K>(&w[0]).borrow());
            let fb = K::as_bytes(first_key::<K>(&w[1]).borrow());
            matches!(K::compare(la.as_ref(), fb.as_ref()), Ordering::Less | Ordering::Equal)
        });

        if disjoint_chain {
            let total: usize = runs.iter().map(|r| r.len()).sum();
            let mut out = Vec::with_capacity(total);
            for mut r in runs {
                out.extend(r.drain(..));
            }
            return out;
        }

        // 4) Balanced reduction by length using a min-heap of (len, idx).
        //    Keep vectors in a separate pool; the heap never compares Vecs.
        let mut pool: Vec<Option<Vec<(K, V)>>> = runs.into_iter().map(Some).collect();
        let mut heap: BinaryHeap<(Reverse<usize>, usize)> = BinaryHeap::new();

        for (i, slot) in pool.iter().enumerate() {
            let len = slot.as_ref().unwrap().len();
            heap.push((Reverse(len), i));
        }

        let mut next_idx = pool.len();

        while heap.len() > 1 {
            let (_, ia) = heap.pop().unwrap();
            let (_, ib) = heap.pop().unwrap();

            // Take ownership of the two smallest runs.
            let a = pool[ia].take().unwrap();
            let b = pool[ib].take().unwrap();

            // Stable two-way merge with your comparator.
            let merged = merge_sorted_by_key_gallop(a, b);

            // Store back into the pool under a fresh index; push by its length.
            pool.push(Some(merged));
            let new_len = pool.last().as_ref().unwrap().as_ref().unwrap().len();
            heap.push((Reverse(new_len), next_idx));
            next_idx += 1;
        }

        // One run remains; return it.
        let (_, idx) = heap.pop().unwrap();
        pool[idx].take().unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn runs(&self) -> usize {
        self.levels.iter().filter(|s| s.is_some()).count()
    }
}

/// lower_bound: first index in `s[0..n)` with key >= pivot
#[inline]
fn gallop_right_slice<K, V>(s: &[(K, V)], n: usize, pivot: &K) -> usize
where
    K: Key + Borrow<K::SelfType<'static>> + 'static,
{
    if n == 0 { return 0; }
    let pb = K::as_bytes(pivot.borrow());

    // Fast check: if first elem already >= pivot, lower_bound is 0.
    {
        let kb0 = K::as_bytes(s[0].0.borrow());
        if !matches!(K::compare(kb0.as_ref(), pb.as_ref()), Ordering::Less) {
            return 0;
        }
    }

    // Exponential search for last index that is < pivot
    let mut last = 0usize;  // s[last] < pivot (invariant)
    let mut ofs  = 1usize;
    while last + ofs < n {
        let idx = last + ofs;
        let kb  = K::as_bytes(s[idx].0.borrow());
        if matches!(K::compare(kb.as_ref(), pb.as_ref()), Ordering::Less) {
            last += ofs;
            ofs <<= 1;
        } else {
            break;
        }
    }
    let mut lo = last + 1;                 // first candidate that could be >= pivot
    let mut hi = (last + ofs).min(n);      // exclusive

    // Binary search in [lo, hi)
    while lo < hi {
        let mid = lo + ((hi - lo) >> 1);
        let kb  = K::as_bytes(s[mid].0.borrow());
        if matches!(K::compare(kb.as_ref(), pb.as_ref()), Ordering::Less) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// upper_bound: first index in `s[0..n)` with key > pivot
#[inline]
fn gallop_left_slice<K, V>(s: &[(K, V)], n: usize, pivot: &K) -> usize
where
    K: Key + Borrow<K::SelfType<'static>> + 'static,
{
    if n == 0 { return 0; }
    let pb = K::as_bytes(pivot.borrow());

    // Fast check: if first elem already > pivot, upper_bound is 0.
    {
        let kb0 = K::as_bytes(s[0].0.borrow());
        if matches!(K::compare(kb0.as_ref(), pb.as_ref()), Ordering::Greater) {
            return 0;
        }
    }

    // Exponential search for last index that is <= pivot
    let mut last = 0usize;  // s[last] <= pivot (invariant)
    let mut ofs  = 1usize;
    while last + ofs < n {
        let idx = last + ofs;
        let kb  = K::as_bytes(s[idx].0.borrow());
        if !matches!(K::compare(kb.as_ref(), pb.as_ref()), Ordering::Greater) {
            last += ofs;
            ofs <<= 1;
        } else {
            break;
        }
    }
    let mut lo = last + 1;                 // first candidate that could be > pivot
    let mut hi = (last + ofs).min(n);      // exclusive

    // Binary search in [lo, hi)
    while lo < hi {
        let mid = lo + ((hi - lo) >> 1);
        let kb  = K::as_bytes(s[mid].0.borrow());
        if !matches!(K::compare(kb.as_ref(), pb.as_ref()), Ordering::Greater) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

pub fn merge_sorted_by_key_gallop<K, V>(a: Vec<(K, V)>, b: Vec<(K, V)>) -> Vec<(K, V)>
where
    K: DbKey,
    V: DbVal,
{
    use std::cmp::Ordering;

    let na = a.len();
    let nb = b.len();
    if na == 0 { return b; }
    if nb == 0 { return a; }

    // Disjoint fast paths (borrows scoped; then move)
    let a_before_b = {
        let a_last_b  = K::as_bytes(a[na - 1].0.borrow());
        let b_first_b = K::as_bytes(b[0].0.borrow());
        matches!(K::compare(a_last_b.as_ref(), b_first_b.as_ref()), Ordering::Less | Ordering::Equal)
    };
    if a_before_b {
        let mut out = a;
        out.reserve(nb);
        out.extend(b);
        return out;
    }
    let b_before_a = {
        let b_last_b  = K::as_bytes(b[nb - 1].0.borrow());
        let a_first_b = K::as_bytes(a[0].0.borrow());
        matches!(K::compare(b_last_b.as_ref(), a_first_b.as_ref()), Ordering::Less)
    };
    if b_before_a {
        let mut out = b;
        out.reserve(na);
        out.extend(a);
        return out;
    }

    // General overlapping: consume via into_iter + scoped slice lookahead
    let mut ia = a.into_iter();
    let mut ib = b.into_iter();
    let mut out: Vec<(K, V)> = Vec::with_capacity(na + nb);

    loop {
        // ---- A block: take A[..] where A_key <= head(B)  (upper_bound) ----
        let end_a = {
            let bs = ib.as_slice();
            if bs.is_empty() { break; }
            let as_ = ia.as_slice();
            if as_.is_empty() { break; }
            let pivot_b = &bs[0].0;
            gallop_left_slice::<K, V>(as_, as_.len(), pivot_b)
        };
        if end_a != 0 {
            out.reserve(end_a);
            for _ in 0..end_a { out.push(ia.next().unwrap()); }
        }

        // ---- B block: take B[..] where B_key < head(A)   (lower_bound) ----
        let end_b = {
            let as_ = ia.as_slice();
            if as_.is_empty() { break; }
            let bs = ib.as_slice();
            if bs.is_empty() { break; }
            let pivot_a = &as_[0].0;
            gallop_right_slice::<K, V>(bs, bs.len(), pivot_a)
        };
        if end_b != 0 {
            out.reserve(end_b);
            for _ in 0..end_b { out.push(ib.next().unwrap()); }
        }
    }

    out.extend(ia);
    out.extend(ib);
    out
}

#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use super::*;
    use crate::storage::test_utils::{addr, Address};

    // --- helpers (kill duplication) ---

    #[inline]
    fn sort_u32_addr_by_key(v: &mut Vec<(u32, Address)>) {
        use redb::{Key as DbVal, Value as RedbValue};
        v.sort_by(|(x,_),(y,_)| {
            let xb = <u32 as RedbValue>::as_bytes(x);
            let yb = <u32 as RedbValue>::as_bytes(y);
            <u32 as DbVal>::compare(xb.as_ref(), yb.as_ref())
        });
    }

    #[inline]
    fn assert_sorted_u32_addr(out: &[(u32, Address)]) {
        use std::cmp::Ordering;
        for w in out.windows(2) {
            let (ka, _)= &w[0];
            let (kb, _)= &w[1];
            let ord = u32::compare(<u32 as redb::Value>::as_bytes(ka).as_ref(), <u32 as redb::Value>::as_bytes(kb).as_ref());
            assert!(matches!(ord, Ordering::Less | Ordering::Equal), "not sorted: {} then {}", ka, kb);
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
            q.push_back(merge_sorted_by_key_gallop(lhs, rhs));
        }
        let out = q.pop_front().unwrap();
        assert_sorted_u32_addr(&out);
        let keys: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(keys, vec![1,2,3,4,5,6,7,8,10]);
    }

    #[test]
    fn merge_buffer_unsorted_chunks_then_take_sorted_gallop_ok() {
        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();

        // three tiny unsorted chunks; values tagged to make equal-keys visible
        mb.merge_unsorted(vec![(5, addr(&[1])), (2, addr(&[1])), (7, addr(&[1]))]);
        mb.merge_unsorted(vec![(3, addr(&[2])), (8, addr(&[2])), (1, addr(&[2]))]);
        mb.merge_unsorted(vec![(6, addr(&[3])), (10, addr(&[3])), (4, addr(&[3]))]);

        // correctness only: globally sorted and all keys present
        let out = mb.take_sorted();
        assert_sorted_u32_addr(&out);
        let keys: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(keys, vec![1,2,3,4,5,6,7,8,10], "unexpected key set/order");
        assert_eq!(mb.runs(), 0, "buffer should be drained after take_sorted()");
    }

    // ===== 2) replacement for “append_sorted_multiple_runs_delegates_to_merge_sorted” =====

    #[test]
    fn append_sorted_multiple_runs_then_drain_gallop_ok() {
        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();

        // two sorted runs (possibly overlapping)
        let mut r1 = vec![(2, addr(&[1])), (5, addr(&[1]))];
        let mut r2 = vec![(1, addr(&[2])), (9, addr(&[2]))];
        sort_u32_addr_by_key(&mut r1);
        sort_u32_addr_by_key(&mut r2);

        mb.merge_sorted(r1);
        mb.merge_sorted(r2);

        // append another sorted run; may become append or deferred merge depending on levels
        let mut r3 = vec![(3, addr(&[3])), (7, addr(&[3]))];
        sort_u32_addr_by_key(&mut r3);
        mb.append_sorted(r3);

        // final result: union of {2,5}, {1,9}, {3,7} in sorted order
        let out = mb.take_sorted();
        assert_sorted_u32_addr(&out);
        let ks: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(ks, vec![1,2,3,5,7,9]);
        assert_eq!(mb.runs(), 0);
    }

    // ===== 3) golden: gallop merge ≡ simple baseline merge on random-ish data =====

    fn baseline_merge_sorted_u32_addr(
        mut a: Vec<(u32, Address)>,
        mut b: Vec<(u32, Address)>
    ) -> Vec<(u32, Address)> {
        use std::cmp::Ordering;
        let mut out = Vec::with_capacity(a.len() + b.len());
        let ia = 0usize;
        let ib = 0usize;
        while ia < a.len() && ib < b.len() {
            let ord = u32::compare(
                <u32 as redb::Value>::as_bytes(&a[ia].0).as_ref(),
                <u32 as redb::Value>::as_bytes(&b[ib].0).as_ref(),
            );
            match ord {
                Ordering::Less | Ordering::Equal => { out.push(a.remove(ia)); } // stable: take from a on equal
                Ordering::Greater                => { out.push(b.remove(ib)); }
            }
        }
        out.extend(a);
        out.extend(b);
        out
    }

    #[test]
    fn gallop_merge_matches_baseline_on_various_shapes() {
        // Shapes: disjoint, tight overlap, equal keys interleaved
        let cases: Vec<(Vec<(u32, Address)>, Vec<(u32, Address)>)> = vec![
            // disjoint
            {
                let mut a = (0..10).map(|k| (k, addr(&[1]))).collect::<Vec<_>>();
                let mut b = (10..20).map(|k| (k, addr(&[2]))).collect::<Vec<_>>();
                sort_u32_addr_by_key(&mut a);
                sort_u32_addr_by_key(&mut b);
                (a,b)
            },
            // overlapping
            {
                let mut a = vec![(1, addr(&[1])), (4, addr(&[1])), (7, addr(&[1])), (9, addr(&[1]))];
                let mut b = vec![(0, addr(&[2])), (3, addr(&[2])), (5, addr(&[2])), (10, addr(&[2]))];
                sort_u32_addr_by_key(&mut a);
                sort_u32_addr_by_key(&mut b);
                (a,b)
            },
            // equal keys: stability check (A-equals should precede B-equals)
            {
                let mut a = vec![(2, addr(&[0xaa])), (5, addr(&[0xaa])), (5, addr(&[0xbb]))];
                let mut b = vec![(2, addr(&[0x11])), (5, addr(&[0x22]))];
                sort_u32_addr_by_key(&mut a);
                sort_u32_addr_by_key(&mut b);
                (a,b)
            },
        ];

        for (a,b) in cases {
            let a1 = a.clone();
            let b1 = b.clone();

            let base = baseline_merge_sorted_u32_addr(a1, b1);
            // use the library gallop merge (u32 implements Key)
            let got  = merge_sorted_by_key_gallop::<u32, Address>(a, b);

            assert_sorted_u32_addr(&got);
            assert_eq!(got.len(), base.len());
            // compare key sequence equality
            let k_base: Vec<u32> = base.iter().map(|(k,_)| *k).collect();
            let k_got:  Vec<u32> = got.iter().map(|(k,_)| *k).collect();
            assert_eq!(k_got, k_base, "gallop result differs from baseline keys");

            // stability on duplicated keys: group keys and check A-before-B by tag
            // (we used different Address byte tags per side).
            // For a rigorous check, locate equal-key blocks and ensure the first occurrences come from A for both.
            // Quick sanity: same multiset of values
            use std::collections::BTreeMap;
            let mut m1 = BTreeMap::new();
            for (_k,v) in base.iter() { *m1.entry(v.0.clone()).or_insert(0usize) += 1; }
            let mut m2 = BTreeMap::new();
            for (_k,v) in got.iter()  { *m2.entry(v.0.clone()).or_insert(0usize) += 1; }
            assert_eq!(m1, m2, "value multiset mismatch");
        }
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

        mb.merge_sorted(r1);
        assert_eq!(mb.runs(), 1, "one sorted run → one level occupied");

        mb.merge_sorted(r2);
        assert_eq!(mb.runs(), 1, "two level-0 runs must carry-merge into one higher run");

        mb.merge_sorted(r3);
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
        mb.merge_sorted(r_old);
        assert_eq!(mb.runs(), 1);
        mb.merge_sorted(r_new);
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


    #[test]
    fn append_sorted_into_empty_creates_one_run() {
        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();
        let mut run = vec![(2, addr(&[1])), (1, addr(&[1])), (3, addr(&[1]))];
        sort_u32_addr_by_key(&mut run);

        mb.append_sorted(run.clone());
        assert_eq!(mb.runs(), 1, "empty buffer + append_sorted => exactly one run");

        let out = mb.take_sorted();
        assert_sorted_u32_addr(&out);
        let ks: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(ks, vec![1,2,3]);
        assert_eq!(mb.runs(), 0);
    }

    #[test]
    fn append_sorted_single_run_fast_path_non_overlapping() {
        // existing run: [1,3,5], new run: [6,7] => pure append (O(m))
        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();
        let mut base = vec![(1, addr(&[1])), (3, addr(&[1])), (5, addr(&[1]))];
        sort_u32_addr_by_key(&mut base);
        mb.append_sorted(base);
        assert_eq!(mb.runs(), 1);

        let mut add = vec![(6, addr(&[2])), (7, addr(&[2]))];
        sort_u32_addr_by_key(&mut add);
        mb.append_sorted(add);

        assert_eq!(mb.runs(), 1, "still a single run after pure append");
        let out = mb.take_sorted();
        assert_sorted_u32_addr(&out);
        let ks: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(ks, vec![1,3,5,6,7]);
    }

    #[test]
    fn append_sorted_single_run_overlap_triggers_one_merge() {
        // existing run: [1,4,8], new run: [3,5] => must merge (O(n+m))
        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();
        let mut base = vec![(1, addr(&[1])), (4, addr(&[1])), (8, addr(&[1]))];
        sort_u32_addr_by_key(&mut base);
        mb.append_sorted(base);
        assert_eq!(mb.runs(), 1);

        let mut add = vec![(3, addr(&[2])), (5, addr(&[2]))];
        sort_u32_addr_by_key(&mut add);
        mb.append_sorted(add);

        assert_eq!(mb.runs(), 1, "merge keeps a single run");
        let out = mb.take_sorted();
        assert_sorted_u32_addr(&out);
        let ks: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(ks, vec![1,3,4,5,8]);
    }

    #[test]
    fn append_sorted_equal_boundary_is_pure_append() {
        // existing tail == new head → stability + append
        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();
        let mut base = vec![(1, addr(&[1])), (3, addr(&[1]))];
        let mut add  = vec![(3, addr(&[2])), (4, addr(&[2]))];
        sort_u32_addr_by_key(&mut base);
        sort_u32_addr_by_key(&mut add);

        mb.append_sorted(base);
        assert_eq!(mb.runs(), 1);
        mb.append_sorted(add); // equal-boundary append

        let out = mb.take_sorted();
        let ks: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(ks, vec![1,3,3,4]); // stability preserved
    }

    #[inline]
    fn assert_sorted(v: &[(u32, Address)]) {
        use redb::{Key as DbVal, Value as RedbValue};
        use std::cmp::Ordering;
        for w in v.windows(2) {
            let (a, _) = &w[0]; let (b, _) = &w[1];
            let ab = <u32 as RedbValue>::as_bytes(a);
            let bb = <u32 as RedbValue>::as_bytes(b);
            let ord = <u32 as DbVal>::compare(ab.as_ref(), bb.as_ref());
            assert!(matches!(ord, Ordering::Less | Ordering::Equal));
        }
    }

    #[test]
    fn append_sorted_empty_then_many_small_after_global_max() {
        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();

        // base run [1,3,5]
        let mut base = vec![(5, addr(&[1])), (1, addr(&[1])), (3, addr(&[1]))];
        sort_u32_addr_by_key(&mut base);
        mb.append_sorted(base);
        assert_eq!(mb.runs(), 1);

        // synthesize several runs that start after the current max (5)
        let mut add1 = vec![(6, addr(&[2])), (7, addr(&[2]))];
        let mut add2 = vec![(8, addr(&[3]))];
        let mut add3 = vec![(9, addr(&[4])), (10, addr(&[4]))];
        sort_u32_addr_by_key(&mut add1);
        sort_u32_addr_by_key(&mut add2);
        sort_u32_addr_by_key(&mut add3);

        mb.append_sorted(add1);
        mb.append_sorted(add2);
        mb.append_sorted(add3);

        // still one run; result is [1,3,5,6,7,8,9,10]
        assert_eq!(mb.runs(), 1);
        let out = mb.take_sorted();
        assert_sorted(&out);
        let ks: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(ks, vec![1,3,5,6,7,8,9,10]);
    }

    #[test]
    fn append_sorted_many_runs_but_new_after_global_max_extends_max_run() {
        use crate::storage::test_utils::{addr, Address};

        #[inline]
        fn assert_sorted(v: &[(u32, Address)]) {
            use redb::{Key as DbVal, Value as RedbValue};
            use std::cmp::Ordering;
            for w in v.windows(2) {
                let (a,_) = &w[0]; let (b,_) = &w[1];
                let ab = <u32 as RedbValue>::as_bytes(a);
                let bb = <u32 as RedbValue>::as_bytes(b);
                assert!(matches!(<u32 as DbVal>::compare(ab.as_ref(), bb.as_ref()),
                             Ordering::Less | Ordering::Equal));
            }
        }

        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();

        // two sorted runs; depending on sizes, buffer may have 1 run (carried) or 2 runs
        let mut r1 = vec![(2, addr(&[1])), (5, addr(&[1]))];
        let mut r2 = vec![(7, addr(&[2])), (9, addr(&[2]))];
        sort_u32_addr_by_key(&mut r1);
        sort_u32_addr_by_key(&mut r2);
        mb.merge_sorted(r1);
        mb.merge_sorted(r2);
        assert!(mb.runs() >= 1);

        // new run strictly after global max (>=9) → should be appended, not merged/carry
        let mut add = vec![(10, addr(&[3])), (11, addr(&[3]))];
        sort_u32_addr_by_key(&mut add);
        mb.append_sorted(add);
        let runs_after  = mb.runs();
        assert!(runs_after >= 1);

        let out = mb.take_sorted();
        assert_sorted(&out);
        let ks: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(ks, vec![2,5,7,9,10,11]);
    }

    #[test]
    fn append_sorted_equal_boundary_is_append_not_merge() {
        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();

        let mut base = vec![(1, addr(&[1])), (3, addr(&[1]))];
        let mut add  = vec![(3, addr(&[2])), (4, addr(&[2]))];
        sort_u32_addr_by_key(&mut base);
        sort_u32_addr_by_key(&mut add);

        mb.append_sorted(base);
        mb.append_sorted(add);

        let out = mb.take_sorted();
        let ks: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(ks, vec![1,3,3,4]);
    }

    #[test]
    fn mix_append_sorted_and_merge_unsorted_interleaved() {
        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();

        // 1) start with a base sorted run: [2,5,9]
        let mut base = vec![(5, addr(&[1])), (2, addr(&[1])), (9, addr(&[1]))];
        sort_u32_addr_by_key(&mut base);
        mb.append_sorted(base);
        assert_eq!(mb.runs(), 1);

        // 2) append_sorted non-overlapping tail: [10,11]  → pure O(m) extend
        let mut after_tail = vec![(11, addr(&[2])), (10, addr(&[2]))];
        sort_u32_addr_by_key(&mut after_tail);
        mb.append_sorted(after_tail);
        assert_eq!(mb.runs(), 1);

        // 3) merge_unsorted overlapping tiny chunk (unsorted): {3,7,10}
        //    - 10 equals boundary → allowed; this will trigger a local merge.
        mb.merge_unsorted(vec![(7, addr(&[3])), (10, addr(&[3])), (3, addr(&[3]))]);

        // 4) append_sorted strictly after global max: [12,13,14] → O(m) extend to max-run
        let mut far_tail = vec![(14, addr(&[4])), (12, addr(&[4])), (13, addr(&[4]))];
        sort_u32_addr_by_key(&mut far_tail);
        mb.append_sorted(far_tail);

        // 5) a couple more unsorted chunks mixed in
        mb.merge_unsorted(vec![(1, addr(&[9])), (4, addr(&[9]))]);
        mb.merge_unsorted(vec![(6, addr(&[8]))]);

        // Final output must be globally sorted; contents are the union of all keys above.
        let out = mb.take_sorted();
        assert_sorted_u32_addr(&out);
        let keys: Vec<u32> = out.iter().map(|(k,_)| *k).collect();
        assert_eq!(keys, vec![1,2,3,4,5,6,7,9,10,10,11,12,13,14]);
        assert_eq!(mb.runs(), 0);
    }

    #[test]
    fn mix_equal_boundary_stability_is_preserved() {
        let mut mb: MergeBuffer<u32, Address> = MergeBuffer::new();

        let a1 = addr(&[0x01]); let a2 = addr(&[0x02]);
        let b1 = addr(&[0x11]); let b2 = addr(&[0x12]);
        let c1 = addr(&[0x21]); let c2 = addr(&[0x22]);

        // base: [5(a1),5(a2)]  — build in desired order
        let mut base = vec![(5, a1.clone()), (5, a2.clone())];
        sort_u32_addr_by_key(&mut base); // stable, preserves a1,a2
        mb.append_sorted(base);

        // append_sorted on equal boundary: [5(b1),5(b2)]  — desired order
        let mut eq_append = vec![(5, b1.clone()), (5, b2.clone())];
        sort_u32_addr_by_key(&mut eq_append); // preserves b1,b2
        mb.append_sorted(eq_append);

        // merge_unsorted with equal keys too: [5(c1),5(c2)] — desired order
        // merge_unsorted sorts stably, so c1,c2 preserved
        mb.merge_unsorted(vec![(5, c1.clone()), (5, c2.clone())]);

        let out = mb.take_sorted();
        let vals_5: Vec<Vec<u8>> = out.iter()
            .filter(|(k,_)| *k == 5)
            .map(|(_,a)| a.0.clone())
            .collect();

        assert_eq!(vals_5, vec![a1.0, a2.0, b1.0, b2.0, c1.0, c2.0]);
    }
}

#[cfg(all(test, feature = "bench"))]
mod bench {
    extern crate test;
    use crate::storage::sort_buffer::MergeBuffer;
    use crate::storage::test_utils::TxHash;
    use test::Bencher;

    // =========================
    // Bench configuration knobs
    // =========================
    // keep all benches around ~30k total pairs
    const TOTAL_TARGET: usize = 30_000;

    // merge-only
    const BASE_LEN_MERGE_ONLY: usize     = 20_000;
    const CHUNK_LEN_UNSORTED: usize      = 5;
    const UNSORTED_RUNS_MERGE_ONLY: usize = (TOTAL_TARGET - BASE_LEN_MERGE_ONLY) / CHUNK_LEN_UNSORTED; // 6_000

    // append-only (balanced baseline)
    const BASE_LEN_APPEND_ONLY: usize     = 20_000;
    const CHUNK_LEN_APPEND_ONLY: usize    = 5;
    const APPEND_RUNS_APPEND_ONLY: usize  = (TOTAL_TARGET - BASE_LEN_APPEND_ONLY) / CHUNK_LEN_APPEND_ONLY; // 2_000

    // mixed bench (balanced)
    const BASE_LEN_MIXED: usize           = 20_000;
    const CHUNK_LEN_MIXED_SORTED: usize   = 5;
    const APPEND_RUNS_MIXED: usize        = 1_000; // 1k * 5 = 5k
    const CHUNK_LEN_MIXED_UNSORTED: usize = 5;
    const UNSORTED_RUNS_MIXED: usize      = 1_000; // 1k * 5 = 5k

    // >>> requested variations with bigger differences <<<
    // Variation A: single-run fast path with ONE BIG append of 5,000
    const BASE_LEN_SINGLE_FAST: usize     = TOTAL_TARGET - 5_000; // 25_000
    const CHUNK_LEN_SINGLE_FAST: usize    = 5_000;

    const CHUNK_LEN_BIG: usize = 5_000;

    // Variation B: many small appends after big base (2,000 * 5)
    const BASE_LEN_MANY_SMALL: usize      = 20_000;
    const CHUNK_LEN_MANY_SMALL: usize     = 5;
    const APPEND_RUNS_MANY_SMALL: usize   = (TOTAL_TARGET - BASE_LEN_MANY_SMALL) / CHUNK_LEN_MANY_SMALL; // 2_000

    // --------------------- Bench knobs (≈30k total) ---------------------

    // Disjoint-chain case: 60 runs × 500 = 30_000 elements
    const R_DISJOINT: usize = 60;
    const L_DISJOINT: usize = 500;

    // Overlap-equal case: 30 runs × 1_000 = 30_000; 50% overlap step = 500
    const R_EQUAL: usize = 30;
    const L_EQUAL: usize = 1_000;
    const STEP_EQUAL: usize = 500; // 50% overlap (start every 500, len 1000)

    // Mixed case: one big 10k + 100 small 200 (with overlap against big) = 30_000 total
    const HUGE_LEN: usize   = 10_000;
    const SMALL_RUNS: usize = 100;
    const MEDIUM_LEN: usize  = 200;
    const SMALL_STEP: usize = 100; // 50% overlap among smalls; all overlap into big band too

    const BASE_LEN: usize   = 1_000; // from 20_000 → keeps scale but light

    // Many tiny overlapping batches (Bitcoin-like “normal” inputs)
    const SMALL_LEN: usize  = 10;
    const N_SMALL: usize    = 30;    // divisible by N_BIG, nice schedule
    const SMALL_BACK: usize = 32;    // start 32 keys before tail → guaranteed overlap

    // Occasional big “dust sweep” (overlapping)
    const BIG_LEN: usize    = 1_000; // one big chunk
    const N_BIG: usize      = 1;     // exactly one sweep
    const BIG_BACK: usize   = 500;   // overlap ~half its length

    // Interleave: after every 30th small, inject the big
    const BIG_EVERY: usize  = N_SMALL / N_BIG; // 30

    // ===========
    // PRNG helpers
    // ===========
    #[inline]
    fn splitmix64(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    // non-monotonic TxHash from a seed (for unsorted runs)
    #[inline]
    fn txhash_from_seed(mut s: u64) -> TxHash {
        let mut bytes = [0u8; 32];
        for i in 0..32 {
            s = splitmix64(s);
            bytes[i] = (s >> ((i & 7) * 8)) as u8;
        }
        TxHash(bytes)
    }

    // Monotonic TxHash stream (big-endian counter) for append fast-path
    #[inline]
    fn bump(seed: &mut [u8; 32]) -> TxHash {
        for i in (0..32).rev() {
            let (v, c) = seed[i].overflowing_add(1);
            seed[i] = v;
            if !c { break; }
        }
        TxHash(*seed)
    }

    // ====== overlap stress helpers (deterministic counters) ======

    #[inline]
    fn seed_from_counter(counter: usize) -> [u8; 32] {
        // place counter in the last 8 bytes (big-endian), rest zero
        let mut s = [0u8; 32];
        let c = counter as u64;
        s[24] = (c >> 56) as u8;
        s[25] = (c >> 48) as u8;
        s[26] = (c >> 40) as u8;
        s[27] = (c >> 32) as u8;
        s[28] = (c >> 24) as u8;
        s[29] = (c >> 16) as u8;
        s[30] = (c >> 8)  as u8;
        s[31] = (c >> 0)  as u8;
        s
    }

    /// Create a MergeBuffer with each provided run placed as a separate level slot.
    fn mk_buf_from_runs(runs: Vec<Vec<(TxHash, u32)>>) -> MergeBuffer<TxHash, u32> {
        MergeBuffer { levels: runs.into_iter().map(Some).collect() }
    }

    // Build a sorted run whose keys are *contiguous counters*, starting at `start_ctr`.
    // This guarantees global order by numeric counter, and lets us force overlap.
    fn build_counter_sorted_run(len: usize, start_ctr: usize, start_val: u32) -> Vec<(TxHash, u32)> {
        let mut out = Vec::with_capacity(len);
        for i in 0..len {
            let seed = seed_from_counter(start_ctr + i);
            out.push((TxHash(seed), start_val.wrapping_add(i as u32)));
        }
        out
    }

    // Stable merge for the eager baseline (compare via redb API).
    fn merge_two_sorted_eager(mut a: Vec<(TxHash, u32)>, mut b: Vec<(TxHash, u32)>)
                              -> Vec<(TxHash, u32)>
    {
        use redb::{Key as DbVal, Value as RedbValue};
        use std::cmp::Ordering;
        let mut out = Vec::with_capacity(a.len() + b.len());
        let mut ia = 0usize;
        let mut ib = 0usize;
        while ia < a.len() && ib < b.len() {
            let (ka, _) = &a[ia];
            let (kb, _) = &b[ib];
            let ab = <TxHash as RedbValue>::as_bytes(ka);
            let bb = <TxHash as RedbValue>::as_bytes(kb);
            match <TxHash as DbVal>::compare(ab.as_ref(), bb.as_ref()) {
                Ordering::Less | Ordering::Equal => { out.push(a[ia].clone()); ia += 1; }
                Ordering::Greater                => { out.push(b[ib].clone()); ib += 1; }
            }
        }
        if ia < a.len() { out.extend(a.drain(ia..)); }
        if ib < b.len() { out.extend(b.drain(ib..)); }
        out
    }

    // ==============
    // Data builders
    // ==============
    fn build_unsorted(len: usize, seed0: u64) -> Vec<(TxHash, u32)> {
        let mut s = seed0 ^ 0xCAFEBABE_DEADBEEF;
        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            let key = txhash_from_seed(s);
            s = splitmix64(s);
            let val = (s as u32).rotate_left(7) ^ 0xA5A5_5A5A;
            v.push((key, val));
        }
        v
    }

    fn build_unsorted_batches(n_batches: usize, len: usize, seed0: u64) -> Vec<Vec<(TxHash, u32)>> {
        (0..n_batches)
            .scan(seed0, |st, _| { *st = splitmix64(*st); Some(*st) })
            .map(|s| build_unsorted(len, s))
            .collect()
    }

    fn build_sorted_run(len: usize, start_seed: [u8; 32], start_val: u32) -> Vec<(TxHash, u32)> {
        let mut s = start_seed;
        let mut out = Vec::with_capacity(len);
        let mut val = start_val;
        for _ in 0..len {
            out.push((bump(&mut s), val));
            val = val.wrapping_add(1);
        }
        out
    }

    fn seed_advance(mut s: [u8; 32], n: usize) -> [u8; 32] {
        for _ in 0..n { let _ = bump(&mut s); }
        s
    }

    // ============================
    // Benches (similar work sizes)
    // ============================

    #[bench]
    fn bench_merge_only_roughly_2000_by_5(b: &mut Bencher) {
        let batches = build_unsorted_batches(UNSORTED_RUNS_MERGE_ONLY, CHUNK_LEN_UNSORTED, 0xD00D_F00D_1234_5678);

        b.iter(|| {
            let mut mb: MergeBuffer<TxHash, u32> = MergeBuffer::new();
            for batch in batches.iter() {
                mb.merge_unsorted(batch.clone()); // sorts + carries
            }
            test::black_box(mb.take_sorted()); // final reduction
        });
    }

    /// Append-only: base 20k + 2,000 chunks × 5 = ~30k pairs; all appends strictly after tail.
    #[bench]
    fn bench_append_only_roughly_6000_by_5(b: &mut Bencher) {
        let base = build_sorted_run(BASE_LEN_APPEND_ONLY, [0u8; 32], 0);

        // small sorted runs strictly after base tail
        let mut tail = [0u8; 32];
        for _ in 0..BASE_LEN_APPEND_ONLY { let _ = bump(&mut tail); }
        let small_sorted: Vec<Vec<(TxHash, u32)>> =
            (0..APPEND_RUNS_APPEND_ONLY)
                .map(|i| {
                    let start = seed_advance(tail, i * CHUNK_LEN_APPEND_ONLY);
                    build_sorted_run(CHUNK_LEN_APPEND_ONLY, start, i as u32)
                })
                .collect();

        b.iter(|| {
            let mut mb = MergeBuffer::from_sorted(base.clone());
            for r in small_sorted.iter() {
                mb.append_sorted(r.clone()); // pure O(m) extend
            }
            test::black_box(mb.take_sorted());
        });
    }

    /// Mixed: base 20k + 1,000 sorted appends (5 each) + 1,000 unsorted merges (5 each) = ~30k pairs.
    #[bench]
    fn bench_mixed_append_and_merge_roughly_30k(b: &mut Bencher) {
        let base = build_sorted_run(BASE_LEN_MIXED, [0u8; 32], 0);

        let mut tail = [0u8; 32];
        for _ in 0..BASE_LEN_MIXED { let _ = bump(&mut tail); }

        let sorted_runs: Vec<Vec<(TxHash, u32)>> =
            (0..APPEND_RUNS_MIXED)
                .map(|i| {
                    let start = seed_advance(tail, i * CHUNK_LEN_MIXED_SORTED);
                    build_sorted_run(CHUNK_LEN_MIXED_SORTED, start, i as u32)
                })
                .collect();

        let unsorted_runs: Vec<Vec<(TxHash, u32)>> =
            (0..UNSORTED_RUNS_MIXED)
                .scan(0xDEAD_BEEF_CAFE_BABE, |st, _| { *st = splitmix64(*st); Some(*st) })
                .map(|s| build_unsorted(CHUNK_LEN_MIXED_UNSORTED, s))
                .collect();

        b.iter(|| {
            let mut mb = MergeBuffer::from_sorted(base.clone());
            for i in 0..APPEND_RUNS_MIXED {
                mb.append_sorted(sorted_runs[i].clone());    // extend
                mb.merge_unsorted(unsorted_runs[i].clone()); // sort+carry
            }
            test::black_box(mb.take_sorted());
        });
    }

    // ---------------------------------------
    // >>> Requested variations (bigger diff)
    // ---------------------------------------

    /// Variation A: **one big append** of 5,000 after a 25k base (fast path).
    #[bench]
    fn bench_append_sorted_single_run_fast_path(b: &mut Bencher) {
        // base 25k
        let base = build_sorted_run(BASE_LEN_SINGLE_FAST, [0u8; 32], 0);

        // a SINGLE 5,000-long sorted run strictly after tail
        let mut tail = [0u8; 32];
        for _ in 0..BASE_LEN_SINGLE_FAST { let _ = bump(&mut tail); }
        let big_append = build_sorted_run(CHUNK_LEN_SINGLE_FAST, tail, 123);

        b.iter(|| {
            let mut mb = MergeBuffer::from_sorted(base.clone());
            mb.append_sorted(big_append.clone()); // pure O(m) extend of 5k
            test::black_box(mb.take_sorted());
        });
    }

    /// Variation B: **many small appends** (2,000 × 5) after a 20k base (fast path).
    #[bench]
    fn bench_append_sorted_many_small_after_big(b: &mut Bencher) {
        // base 20k
        let base = build_sorted_run(BASE_LEN_MANY_SMALL, [0u8; 32], 0);

        // 2,000 tiny runs strictly after base tail
        let mut tail = [0u8; 32];
        for _ in 0..BASE_LEN_MANY_SMALL { let _ = bump(&mut tail); }
        let small_sorted: Vec<Vec<(TxHash, u32)>> =
            (0..APPEND_RUNS_MANY_SMALL)
                .map(|i| {
                    let start = seed_advance(tail, i * CHUNK_LEN_MANY_SMALL);
                    build_sorted_run(CHUNK_LEN_MANY_SMALL, start, i as u32)
                })
                .collect();

        b.iter(|| {
            let mut mb = MergeBuffer::from_sorted(base.clone());
            for r in small_sorted.iter() {
                mb.append_sorted(r.clone()); // many O(m) extends of length=5
            }
            test::black_box(mb.take_sorted());
        });
    }


    /// Append 5 big sorted runs (10k each).
    /// First append installs into empty buffer; next 4 are pure O(m) extends (runs are strictly after tail).
    #[bench]
    fn bench_append_sorted_big_5x(b: &mut Bencher) {
        // Build 5 disjoint sorted runs S0..S4, strictly increasing (fast-path appends).
        let tail = [0u8; 32];
        let sorted_runs: Vec<Vec<(TxHash, u32)>> = (0..5)
            .map(|i| {
                let start = seed_advance(tail, i * CHUNK_LEN_BIG);
                build_sorted_run(CHUNK_LEN_BIG, start, i as u32)
            })
            .collect();

        b.iter(|| {
            let mut mb: MergeBuffer<TxHash, u32> = MergeBuffer::new();
            for r in sorted_runs.iter() {
                mb.append_sorted(r.clone()); // installs on first, then 4 fast extends
            }
            test::black_box(mb.take_sorted());
        });
    }

    /// Merge 5 big unsorted runs (10k each).
    #[bench]
    fn bench_merge_unsorted_big_5x(b: &mut Bencher) {
        // Build 5 unsorted runs U0..U4 (random-ish keys → will overlap).
        let unsorted_runs: Vec<Vec<(TxHash, u32)>> = (0..5)
            .scan(0xDEAD_BEEF_CAFE_BABE_u64, |st, _| { *st = splitmix64(*st); Some(*st) })
            .map(|s| build_unsorted(CHUNK_LEN_BIG, s))
            .collect();

        b.iter(|| {
            let mut mb: MergeBuffer<TxHash, u32> = MergeBuffer::new();
            for u in unsorted_runs.iter() {
                mb.merge_unsorted(u.clone()); // sort + binary-counter carries
            }
            test::black_box(mb.take_sorted());
        });
    }

    /// Mixed: for i in 0..5, append one big sorted run (10k) then merge one big unsorted run (10k).
    #[bench]
    fn bench_mixed_big_5x(b: &mut Bencher) {
        // 5 disjoint sorted runs S0..S4 (strictly after tail for append fast-path).
        let tail = [0u8; 32];
        let sorted_runs: Vec<Vec<(TxHash, u32)>> = (0..5)
            .map(|i| {
                let start = seed_advance(tail, i * CHUNK_LEN_BIG);
                build_sorted_run(CHUNK_LEN_BIG, start, i as u32)
            })
            .collect();

        // 5 unsorted runs U0..U4.
        let unsorted_runs: Vec<Vec<(TxHash, u32)>> = (0..5)
            .scan(0xBADC0FFE_F00D_FACE_u64, |st, _| { *st = splitmix64(*st); Some(*st) })
            .map(|s| build_unsorted(CHUNK_LEN_BIG, s))
            .collect();

        b.iter(|| {
            let mut mb: MergeBuffer<TxHash, u32> = MergeBuffer::new();
            for i in 0..5 {
                mb.append_sorted(sorted_runs[i].clone());  // fast O(m) extend
                mb.merge_unsorted(unsorted_runs[i].clone()); // sort + carry
            }
            test::black_box(mb.take_sorted());
        });
    }

    #[bench]
    fn bench_overlap_eager_merge_big_and_small(b: &mut test::Bencher) {
        // Prebuild with the scaled constants
        let base      = build_counter_sorted_run(BASE_LEN, 0, 0);
        let mut small = Vec::with_capacity(N_SMALL);
        let mut big   = Vec::with_capacity(N_BIG);

        let mut tail_ctr = BASE_LEN;

        for i in 0..N_SMALL {
            let start = tail_ctr.saturating_sub(SMALL_BACK);
            small.push(build_counter_sorted_run(SMALL_LEN, start, i as u32));
            tail_ctr += SMALL_LEN;
        }
        for i in 0..N_BIG {
            let start = tail_ctr.saturating_sub(BIG_BACK);
            big.push(build_counter_sorted_run(BIG_LEN, start, (i as u32) << 16));
            tail_ctr += BIG_LEN;
        }

        b.iter(|| {
            let mut acc = base.clone();
            let mut big_idx = 0usize;

            for i in 0..N_SMALL {
                acc = merge_two_sorted_eager(acc, small[i].clone());
                if (i + 1) % BIG_EVERY == 0 && big_idx < N_BIG {
                    acc = merge_two_sorted_eager(acc, big[big_idx].clone());
                    big_idx += 1;
                }
            }
            test::black_box(&acc);
        });
    }

    /// Deferred: use MergeBuffer + append_sorted with "defer on overlap".
    /// Overlapping runs are queued as separate runs (O(1) now); pure-after-tail runs extend O(m).
    #[bench]
    fn bench_overlap_deferred_append_big_and_small(b: &mut test::Bencher) {
        // Reuse the exact same batches as eager bench
        let base      = build_counter_sorted_run(BASE_LEN, 0, 0);
        let mut small = Vec::with_capacity(N_SMALL);
        let mut big   = Vec::with_capacity(N_BIG);

        let mut tail_ctr = BASE_LEN;

        for i in 0..N_SMALL {
            let start = tail_ctr.saturating_sub(SMALL_BACK);
            small.push(build_counter_sorted_run(SMALL_LEN, start, i as u32));
            tail_ctr += SMALL_LEN;
        }
        for i in 0..N_BIG {
            let start = tail_ctr.saturating_sub(BIG_BACK);
            big.push(build_counter_sorted_run(BIG_LEN, start, (i as u32) << 16));
            tail_ctr += BIG_LEN;
        }

        b.iter(|| {
            let mut mb = MergeBuffer::from_sorted(base.clone());
            let mut big_idx = 0usize;

            for i in 0..N_SMALL {
                // This call will detect overlap and *enqueue* the run instead of merging immediately.
                mb.append_sorted(small[i].clone());

                if (i + 1) % BIG_EVERY == 0 && big_idx < N_BIG {
                    mb.append_sorted(big[big_idx].clone()); // also overlapping → deferred
                    big_idx += 1;
                }
            }

            // Do final reduction once (outside the hot path)
            let out = mb.take_sorted();
            test::black_box(out);
        });
    }

    /// take_sorted() on many non-overlapping runs (concat fast path).
    #[bench]
    fn bench_take_sorted_disjoint_chain_30k(b: &mut Bencher) {
        // Build disjoint runs: run i starts at i * L_DISJOINT, length L_DISJOINT.
        let runs: Vec<_> = (0..R_DISJOINT)
            .map(|i| build_counter_sorted_run(L_DISJOINT, i * L_DISJOINT, i as u32))
            .collect();
        let tmpl = mk_buf_from_runs(runs);

        b.iter(|| {
            // isolate take_sorted cost
            let mut buf = tmpl.clone();
            let out = buf.take_sorted();
            test::black_box(out);
        });
    }

    /// take_sorted() on equal-sized runs with 50% overlap (forces real merging).
    #[bench]
    fn bench_take_sorted_overlap_equal_runs_30k(b: &mut Bencher) {
        // Run i: start at i * STEP_EQUAL, len L_EQUAL (so neighbors overlap by 50%)
        let runs: Vec<_> = (0..R_EQUAL)
            .map(|i| build_counter_sorted_run(L_EQUAL, i * STEP_EQUAL, (i as u32) << 8))
            .collect();
        let tmpl = mk_buf_from_runs(runs);

        b.iter(|| {
            let mut buf = tmpl.clone();
            let out = buf.take_sorted();
            test::black_box(out);
        });
    }

    /// take_sorted() with one big run and many small overlapping runs (merge ordering stress).
    #[bench]
    fn bench_take_sorted_mixed_sizes_overlap_30k(b: &mut Bencher) {
        // Big in the middle band: [10_000 .. 20_000)
        let big = build_counter_sorted_run(HUGE_LEN, 10_000, 0xB1B1_0000);

        // Smalls sprinkled across and overlapping with the big band:
        // start = 10_000 - 5_000 + i*SMALL_STEP  (covers into, across, and beyond big)
        let base_start = 10_000usize.saturating_sub(5_000);
        let smalls: Vec<_> = (0..SMALL_RUNS)
            .map(|i| build_counter_sorted_run(MEDIUM_LEN, base_start + i * SMALL_STEP, 0x51_0000 + i as u32))
            .collect();

        let mut runs = Vec::with_capacity(1 + SMALL_RUNS);
        runs.push(big);
        runs.extend(smalls);
        let tmpl = mk_buf_from_runs(runs);

        b.iter(|| {
            let mut buf = tmpl.clone();
            let out = buf.take_sorted();
            test::black_box(out);
        });
    }
}
