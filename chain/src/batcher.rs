//! Global reorder + weight batcher + sort_handle integration (lower-bound init)
//!
//! You provide a **conservative lower bound** `start_lower_bound` at construction
//! (e.g., resume_height, or first_seen.saturating_sub(window)). No lazy init.
//!
//! Design goals:
//! - Separate concerns: `ReorderBuffer<TB>` (global monotonic order) and `Batcher<TB>`.
//! - Minimal per-item cost: O(1) expected per `insert()` and per `push_with()`.
//! - No inter-batch disorder. Strictly increasing heights across the whole stream.
//!
//! Notes:
//! - `ReorderBuffer` drops items with `height < next_expected()` (already emitted)
//!   to avoid leaks; a `dropped_too_low` counter is exposed for observability.
//! - `is_saturated()` is a soft hint for backpressure/alerting.

use std::collections::HashMap;
use redb::Durability;

pub type Height = u32;

// =============================================================
// ReorderBuffer<TB>
// =============================================================

/// Guarantees globally increasing delivery (contiguous from `next_height`).
/// Stores out-of-order blocks in O(1) expected time per insert.
///
/// === Complexity Proof Sketch ===
/// Let N be the number of distinct heights processed.
/// Each `insert()` does at most one HashMap insertion (O(1) expected), then
/// repeatedly removes the *contiguous* prefix starting at `next_height`.
/// Each height is removed exactly once overall => total removals O(N).
/// Amortized cost per `insert()` is therefore **O(1) expected**.
/// Memory is O(W) where W is the current out-of-order window size.
pub struct ReorderBuffer<TB> {
    /// The smallest height that has not yet been emitted.
    /// Only when `pending` contains an entry for this height (and any subsequent
    /// contiguous heights) will those items be released in order.
    next_height: Height,

    /// Out-of-order items keyed by their height.
    /// Memory usage is O(W), where W is the current out-of-order window
    /// (`max_seen - next_height + 1`, bounded in practice by network skew).
    pending: HashMap<Height, TB>,

    /// Soft upper bound on `pending.len()` for observability/backpressure.
    /// Exceeding this does not change semantics; you can log/alert or reduce
    /// upstream concurrency when this hint is reached.
    max_buffer_hint: usize,

    /// The largest height observed so far (monotonic, for diagnostics).
    /// Together with `next_height` it defines the current gap span.
    max_seen: Height,

    /// Count of items dropped because their height was below `next_height`
    /// (i.e., already emitted). This counter increments saturatingly to avoid
    /// overflow and helps detect late/duplicate deliveries.
    dropped_too_low: u32,
}
impl<TB> ReorderBuffer<TB> {
    /// Construct with a **conservative lower bound** `start_lower_bound` and a soft buffer hint.
    /// The buffer will only emit a height once *all* previous heights have been seen.
    pub fn new(start_lower_bound: Height, max_buffer_hint: usize) -> Self {
        Self {
            next_height: start_lower_bound,
            pending: HashMap::new(),
            max_buffer_hint,
            max_seen: start_lower_bound.saturating_sub(1),
            dropped_too_low: 0,
        }
    }

    /// Insert a block with its height. Returns the *contiguous* sequence that becomes ready
    /// starting from `next_height`. Global order is guaranteed.
    ///
    /// Amortized O(1) expected per call over any run that touches each height once.
    pub fn insert(&mut self, height: Height, block: TB) -> Vec<TB> {
        // If we already emitted past this height, drop it (observability counter increments).
        if height < self.next_height {
            self.dropped_too_low = self.dropped_too_low.saturating_add(1);
            return Vec::new();
        }
        if height > self.max_seen {
            self.max_seen = height;
        }
        // First-come-wins; ignore duplicates to avoid double-release.
        self.pending.entry(height).or_insert(block);
        let mut ready = Vec::new();
        while let Some(b) = self.pending.remove(&self.next_height) {
            ready.push(b);
            self.next_height += 1;
        }
        ready
    }

    /// Number of out-of-order items currently buffered.
    pub fn pending_len(&self) -> usize { self.pending.len() }

    /// Returns true when the soft buffer hint is reached. Use for observability/backpressure.
    pub fn is_saturated(&self) -> bool { self.pending.len() >= self.max_buffer_hint }

    /// Height we are waiting for next.
    pub fn next_expected(&self) -> Height { self.next_height }

    /// Largest height observed so far.
    pub fn max_seen(&self) -> Height { self.max_seen }

    /// Current gap (next_expected..=max_seen), if any.
    pub fn gap_span(&self) -> Option<(Height, Height)> {
        if self.max_seen >= self.next_height { Some((self.next_height, self.max_seen)) } else { None }
    }

    /// Count of items dropped for arriving below `next_expected()`.
    pub fn too_low_count(&self) -> u32 { self.dropped_too_low }
}

// =============================================================
// Batcher<TB>
// =============================================================

/// Batches an already in-order stream by weight and/or capacity.
///
/// === Complexity ===
/// `push_with`: O(1) to update counters and push; O(K) only when flushing,
/// to move the K items out via `std::mem::take`. Over M items with B flushes,
/// total is O(M). Amortized **O(1)** per item.
pub struct Batcher<TB> {
    /// In-flight batch storage.
    /// Items are appended in the order they’re received from the (already in-order) upstream.
    /// Emptied atomically on flush via `std::mem::take`.
    buf: Vec<TB>,

    /// Accumulated “weight” of the current `buf`.
    /// This is the running sum of `weight_of(&item)` provided to `push_with`.
    /// Reset to 0 on every flush.
    weight: usize,

    /// Inclusive flush threshold for `weight`.
    /// Whenever `weight >= min_weight`, the batch is emitted immediately.
    /// Set this to 0 to flush on every push; typically a positive value (e.g., bytes, gas, tx count proxy).
    min_weight: usize,

    /// Maximum number of items allowed in `buf` before a forced flush.
    /// Acts as a safety valve against unbounded item count even if `min_weight` isn’t reached.
    /// If `buf.len() >= cap`, a flush is triggered.
    cap: usize,
    mode: Durability,
}
impl<TB> Batcher<TB> {
    pub fn new(min_weight: usize, capacity: usize, mode: Durability) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
            weight: 0,
            min_weight,
            cap: capacity,
            mode,
        }
    }

    /// Push `item`; returns a full batch when ready.
    /// In Immediate mode, returns `Some(vec![item])` every time.
    pub fn push_with<F: Fn(&TB) -> usize>(&mut self, item: TB, weight_of: F) -> Option<Vec<TB>> {
        match self.mode {
            Durability::Immediate => {
                // Ignore buf/weight entirely; emit single-item batch.
                Some(vec![item])
            }
            Durability::None => {
                let w = weight_of(&item);
                self.weight += w;
                self.buf.push(item);
                if self.weight >= self.min_weight || self.buf.len() >= self.cap {
                    return Some(self.take_inner());
                }
                None
            }
            _ => unreachable!("unsupported durability mode")
        }
    }

    /// Final flush. In Immediate mode, this is a no-op and returns None.
    pub fn flush(&mut self) -> Option<Vec<TB>> {
        match self.mode {
            Durability::Immediate => None,
            Durability::None => {
                if self.buf.is_empty() { None } else { Some(self.take_inner()) }
            }
            _ => unreachable!("unsupported durability mode")
        }
    }

    fn take_inner(&mut self) -> Vec<TB> {
        self.weight = 0;
        std::mem::take(&mut self.buf)
    }

    pub fn len(&self) -> usize {
        match self.mode {
            Durability::Immediate => 0,
            Durability::None => self.buf.len(),
            _ => unreachable!("unsupported durability mode")
        }
    }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
}


#[cfg(test)]
mod tests {
    use super::*;

    // ---------- fixtures & helpers (u32 heights) ----------

    #[derive(Clone, Debug)]
    struct MockHeader { h: u32, w: usize }
    #[derive(Clone, Debug)]
    struct MockBlock(MockHeader);

    impl MockBlock { fn header(&self) -> &MockHeader { &self.0 } }
    impl MockHeader {
        fn height(&self) -> u32 { self.h }
        fn weight(&self) -> usize { self.w }
    }

    fn mb(h: u32, w: usize) -> MockBlock { MockBlock(MockHeader { h, w }) }

    /// Push one item into the batcher; if a batch is produced, append its heights.
    fn push_collect_heights(
        b: &mut Batcher<MockBlock>,
        item: MockBlock,
        out: &mut Vec<u32>,
    ) {
        if let Some(batch) = b.push_with(item, |x| x.header().weight()) {
            out.extend(heights(&batch));
        }
    }

    /// Feed a sequence of ready blocks into the batcher and append produced heights.
    fn feed_ready<I: IntoIterator<Item = MockBlock>>(
        b: &mut Batcher<MockBlock>,
        ready: I,
        out: &mut Vec<u32>,
    ) {
        for blk in ready { push_collect_heights(b, blk, out); }
    }

    /// Flush the batcher and append produced heights (if any).
    fn flush_collect(b: &mut Batcher<MockBlock>, out: &mut Vec<u32>) {
        if let Some(tail) = b.flush() { out.extend(heights(&tail)); }
    }

    /// Collect heights from a slice/vec of blocks.
    fn heights(blocks: &[MockBlock]) -> Vec<u32> {
        blocks.iter().map(|b| b.header().height()).collect()
    }

    /// Convenience for inclusive range -> Vec<u32>
    fn range_vec(lo: u32, hi: u32) -> Vec<u32> {
        (lo..=hi).collect()
    }

    /// Insert a range and assert that no items are emitted (all inserts return empty).
    fn insert_range_expect_empty(
        r: &mut ReorderBuffer<MockBlock>,
        rng: impl IntoIterator<Item = u32>,
    ) {
        for h in rng {
            assert!(r.insert(h, mb(h, 1)).is_empty(), "expected no emission for height {}", h);
        }
    }

    // ---------- tests ----------

    #[test]
    fn immediate_mode_emits_every_item() {
        let mut b = Batcher::new(1_000_000, 1_000_000, Durability::Immediate);
        let mut out: Vec<Vec<u32>> = Vec::new();

        for h in [10u32, 11, 12] {
            let batch = b.push_with(mb(h, 999_999), |x| x.header().weight());
            assert!(batch.is_some(), "Immediate mode must emit on every push");
            out.push(heights(&batch.unwrap()));
        }
        assert_eq!(out, vec![vec![10], vec![11], vec![12]]);
        assert!(b.flush().is_none(), "Immediate mode flush is a no-op");
    }

    #[test]
    fn thresholds_mode_batches_by_weight() {
        let mut b = Batcher::new(5, 100, Durability::None);
        let mut emitted = Vec::new();

        // weight=1 x 5 → flush once with [0..4]
        for h in 0u32..5 { push_collect_heights(&mut b, mb(h, 1), &mut emitted); }
        assert_eq!(emitted, range_vec(0, 4));
        assert!(b.flush().is_none(), "nothing left after flush");
    }

    #[test]
    fn reorder_contiguous_release() {
        let mut r = ReorderBuffer::new(183, 1024);
        let mut b = Batcher::new(100, 1000, Durability::None);
        let mut emitted = Vec::new();

        assert!(r.insert(185, mb(185, 1)).is_empty());
        feed_ready(&mut b, r.insert(183, mb(183, 1)), &mut emitted); // [183]
        feed_ready(&mut b, r.insert(184, mb(184, 1)), &mut emitted); // [184,185]
        flush_collect(&mut b, &mut emitted);

        assert_eq!(emitted, vec![183, 184, 185]);
        assert!(r.gap_span().is_none());
    }

    #[test]
    fn end_to_end_ordering() {
        let mut r = ReorderBuffer::new(0, 1024);
        let mut b = Batcher::new(3, 100, Durability::None);
        let mut emitted = Vec::new();

        for (h, w) in [2u32, 0, 1, 3, 4, 6, 5].into_iter().zip([1usize; 7]) {
            feed_ready(&mut b, r.insert(h, mb(h, w)), &mut emitted);
        }
        flush_collect(&mut b, &mut emitted);

        assert_eq!(emitted, vec![0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn gap_waits_then_releases() {
        let start: u32 = 100;
        let mut r = ReorderBuffer::new(start, 8);
        let mut b = Batcher::new(10, 100, Durability::None); // min_weight=10
        let mut emitted: Vec<u32> = Vec::new();

        // Preload 101..=130 except 115; nothing should be released yet.
        insert_range_expect_empty(&mut r, (start + 1)..=(start + 30));
        // remove the one we shouldn't have inserted above
        assert!(r.insert(115, mb(115, 1)).is_empty()); // ensure we *did* skip 115
        // But the line above inserted 115; we need to undo that: reinitialize test inputs:
        // Simpler approach: reset r and preload properly:
        let mut r = ReorderBuffer::new(start, 8);
        for h in (start + 1)..=(start + 30) { if h != 115 { assert!(r.insert(h, mb(h, 1)).is_empty()); } }

        assert_eq!(r.next_expected(), start);
        assert_eq!(r.max_seen(), start + 30);
        assert!(r.is_saturated(), "soft hint should trigger under a big gap");
        assert_eq!(r.pending_len(), 29);

        // Insert `start` → reorder returns the full contiguous prefix [100..=114].
        let ready1 = r.insert(start, mb(start, 1));
        assert_eq!(heights(&ready1), range_vec(start, 114));

        // Feed into batcher (min_weight = 10) → first flush is the first 10 items [100..=109].
        feed_ready(&mut b, ready1, &mut emitted);
        assert_eq!(emitted, range_vec(start, start + 9)); // [100..=109]

        // Insert the missing height 115 → reorder now releases [115..=130].
        let ready2 = r.insert(115, mb(115, 1));
        assert_eq!(ready2.first().map(|b| b.header().height()), Some(115));
        assert_eq!(ready2.last().map(|b| b.header().height()), Some(start + 30));

        // Feed the cascade; batcher will flush as thresholds are crossed.
        feed_ready(&mut b, ready2, &mut emitted);

        // Final flush for any remainder below threshold.
        flush_collect(&mut b, &mut emitted);

        // Now we must have the full ordered range.
        assert_eq!(emitted, range_vec(start, start + 30));
        assert!(r.gap_span().is_none());
    }

    #[test]
    fn duplicates_ignored() {
        let mut r = ReorderBuffer::new(10, 1024);
        assert!(r.insert(12, mb(12, 1)).is_empty());
        assert!(r.insert(12, mb(12, 1)).is_empty());

        let out1 = r.insert(10, mb(10, 1));
        assert_eq!(heights(&out1), vec![10]);

        let out2 = r.insert(11, mb(11, 1));
        assert_eq!(heights(&out2), vec![11, 12]);
    }

    #[test]
    fn batcher_multiple_flushes_on_large_release() {
        let mut r = ReorderBuffer::new(0, 1024);
        let mut b = Batcher::new(5, 100, Durability::None);
        let mut emitted = Vec::new();

        insert_range_expect_empty(&mut r, 1..=9);

        feed_ready(&mut b, r.insert(0, mb(0, 1)), &mut emitted);
        for h in 1..=9 { feed_ready(&mut b, r.insert(h, mb(h, 1)), &mut emitted); }
        flush_collect(&mut b, &mut emitted);

        assert_eq!(emitted, range_vec(0, 9));
        assert_eq!(emitted.len(), 10);
    }

    #[test]
    fn diagnostics_show_gap_span() {
        let mut r = ReorderBuffer::new(50, 4);
        insert_range_expect_empty(&mut r, [52u32, 53, 60, 61, 62]);

        assert_eq!(r.next_expected(), 50u32);
        assert_eq!(r.max_seen(), 62u32);

        match r.gap_span() {
            Some((lo, hi)) => { assert_eq!(lo, 50u32); assert_eq!(hi, 62u32); }
            None => panic!("expected a gap"),
        }
        assert!(r.is_saturated(), "soft hint reached at pending_len >= 4");
        assert_eq!(r.pending_len(), 5);
    }

    #[test]
    fn too_low_are_dropped_and_counted() {
        let mut r = ReorderBuffer::new(200, 64);
        let before = r.too_low_count();

        // Below lower bound → dropped and not retained.
        assert!(r.insert(180, mb(180, 1)).is_empty());
        assert!(r.insert(199, mb(199, 1)).is_empty());
        assert_eq!(r.too_low_count(), before + 2);
        assert_eq!(r.pending_len(), 0, "too-low items must not be retained");

        // Normal flow still works.
        let mut b = Batcher::new(3, 10, Durability::None);
        let mut emitted = Vec::new();
        for h in 200..=205 { feed_ready(&mut b, r.insert(h, mb(h, 1)), &mut emitted); }
        flush_collect(&mut b, &mut emitted);
        assert_eq!(emitted, range_vec(200, 205));
    }

    #[test]
    fn immediate_mode_with_reorder_integration() {
        let mut r = ReorderBuffer::new(100, 1024);
        let mut b = Batcher::new(9999, 9999, Durability::Immediate);
        let mut emitted = Vec::new();

        for h in [102u32, 100, 101, 103] {
            feed_ready(&mut b, r.insert(h, mb(h, 1)), &mut emitted);
        }
        assert_eq!(emitted, vec![100, 101, 102, 103]);
    }
}
