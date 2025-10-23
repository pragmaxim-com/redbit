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

use redb::Durability;

/// Batches an already in-order stream by weight and/or capacity.
///
/// === Complexity ===
/// `push_with`: O(1) to update counters and push; O(K) only when flushing,
/// to move the K items out via `std::mem::take`. Over M items with B flushes,
/// total is O(M). Amortized **O(1)** per item.
pub struct WeightBatcher<TB> {
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
impl<TB> WeightBatcher<TB> {
    pub fn new(min_weight: usize, cap: usize, mode: Durability) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
            weight: 0,
            min_weight,
            cap,
            mode,
        }
    }

    /// Push `item`; returns a full batch when ready.
    pub fn push_with<F: Fn(&TB) -> usize>(&mut self, item: TB, weight_of: F) -> Option<Vec<TB>> {
        match self.mode {
            Durability::Immediate => {
                Some(vec![item])
            }
            Durability::None => {
                let w = weight_of(&item);
                self.weight = self.weight.saturating_add(w);
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
        std::mem::replace(&mut self.buf, Vec::with_capacity(self.cap))
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
    use crate::reorder_buffer::ReorderBuffer;
    use crate::test_utils;

    #[test]
    fn capacity_preserved_after_flush() {
        let mut b: WeightBatcher<u8> = WeightBatcher::new(10, 64, Durability::None);
        assert!(b.buf.capacity() >= 64); // initial capacity
        for i in 0..5 {
            let _ = b.push_with(i, |_| 3usize); // small pushes
        }
        // force flush by pushing an item that crosses min_weight
        let batch = b.push_with(99u8, |_| 100usize).expect("should flush");
        assert!(!batch.is_empty());
        // after flush the internal buffer should still have capacity == configured cap
        assert_eq!(b.buf.capacity(), 64);
    }

    #[test]
    fn weight_saturates_and_resets_on_flush() {
        let mut b: WeightBatcher<u8> = WeightBatcher::new(usize::MAX, 10, Durability::None);
        let out = b.push_with(1u8, |_| usize::MAX);
        // push_with should have triggered a flush and returned the batch
        assert!(out.is_some());
        // after flush weight is reset to 0
        assert_eq!(b.weight, 0);
    }

    #[test]
    fn immediate_mode_returns_single_item_and_len_zero() {
        let mut b: WeightBatcher<i32> = WeightBatcher::new(10, 10, Durability::Immediate);
        let out = b.push_with(5, |_| 1usize);
        assert_eq!(out, Some(vec![5]));
        assert_eq!(b.len(), 0);
        assert!(b.flush().is_none());
    }

    #[test]
    fn immediate_mode_emits_every_item() {
        let mut b = WeightBatcher::new(1_000_000, 1_000_000, Durability::Immediate);
        let mut out: Vec<Vec<u32>> = Vec::new();

        for h in [10u32, 11, 12] {
            let batch = b.push_with(test_utils::mb(h, 999_999), |x| x.header().weight());
            assert!(batch.is_some(), "Immediate mode must emit on every push");
            out.push(test_utils::heights(&batch.unwrap()));
        }
        assert_eq!(out, vec![vec![10], vec![11], vec![12]]);
        assert!(b.flush().is_none(), "Immediate mode flush is a no-op");
    }

    #[test]
    fn thresholds_mode_batches_by_weight() {
        let mut b = WeightBatcher::new(5, 100, Durability::None);
        let mut emitted = Vec::new();

        // weight=1 x 5 → flush once with [0..4]
        for h in 0u32..5 { test_utils::push_collect_heights(&mut b, test_utils::mb(h, 1), &mut emitted); }
        assert_eq!(emitted, test_utils::range_vec(0, 4));
        assert!(b.flush().is_none(), "nothing left after flush");
    }

    #[test]
    fn reorder_contiguous_release() {
        let mut r = ReorderBuffer::new(183, 1024);
        let mut b = WeightBatcher::new(100, 1000, Durability::None);
        let mut emitted = Vec::new();

        assert!(r.insert(185, test_utils::mb(185, 1)).is_empty());
        test_utils::feed_ready(&mut b, r.insert(183, test_utils::mb(183, 1)), &mut emitted); // [183]
        test_utils::feed_ready(&mut b, r.insert(184, test_utils::mb(184, 1)), &mut emitted); // [184,185]
        test_utils::flush_collect(&mut b, &mut emitted);

        assert_eq!(emitted, vec![183, 184, 185]);
        assert!(r.gap_span().is_none());
    }

    #[test]
    fn end_to_end_ordering() {
        let mut r = ReorderBuffer::new(0, 1024);
        let mut b = WeightBatcher::new(3, 100, Durability::None);
        let mut emitted = Vec::new();

        for (h, w) in [2u32, 0, 1, 3, 4, 6, 5].into_iter().zip([1usize; 7]) {
            test_utils::feed_ready(&mut b, r.insert(h, test_utils::mb(h, w)), &mut emitted);
        }
        test_utils::flush_collect(&mut b, &mut emitted);

        assert_eq!(emitted, vec![0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn gap_waits_then_releases() {
        let start: u32 = 100;
        let mut r = ReorderBuffer::new(start, 8);
        let mut b = WeightBatcher::new(10, 100, Durability::None); // min_weight=10
        let mut emitted: Vec<u32> = Vec::new();

        // Preload 101..=130 except 115; nothing should be released yet.
        test_utils::insert_range_expect_empty(&mut r, (start + 1)..=(start + 30));
        // remove the one we shouldn't have inserted above
        assert!(r.insert(115, test_utils::mb(115, 1)).is_empty()); // ensure we *did* skip 115
        // But the line above inserted 115; we need to undo that: reinitialize test inputs:
        // Simpler approach: reset r and preload properly:
        let mut r = ReorderBuffer::new(start, 8);
        for h in (start + 1)..=(start + 30) { if h != 115 { assert!(r.insert(h, test_utils::mb(h, 1)).is_empty()); } }

        assert_eq!(r.next_expected(), start);
        assert_eq!(r.max_seen(), Some(start + 30));
        assert!(r.is_saturated(), "soft hint should trigger under a big gap");
        assert_eq!(r.pending_len(), 29);

        // Insert `start` → reorder returns the full contiguous prefix [100..=114].
        let ready1 = r.insert(start, test_utils::mb(start, 1));
        assert_eq!(test_utils::heights(&ready1), test_utils::range_vec(start, 114));

        // Feed into batcher (min_weight = 10) → first flush is the first 10 items [100..=109].
        test_utils::feed_ready(&mut b, ready1, &mut emitted);
        assert_eq!(emitted, test_utils::range_vec(start, start + 9)); // [100..=109]

        // Insert the missing height 115 → reorder now releases [115..=130].
        let ready2 = r.insert(115, test_utils::mb(115, 1));
        assert_eq!(ready2.first().map(|b| b.header().height()), Some(115));
        assert_eq!(ready2.last().map(|b| b.header().height()), Some(start + 30));

        // Feed the cascade; batcher will flush as thresholds are crossed.
        test_utils::feed_ready(&mut b, ready2, &mut emitted);

        // Final flush for any remainder below threshold.
        test_utils::flush_collect(&mut b, &mut emitted);

        // Now we must have the full ordered range.
        assert_eq!(emitted, test_utils::range_vec(start, start + 30));
        assert!(r.gap_span().is_none());
    }

    #[test]
    fn batcher_multiple_flushes_on_large_release() {
        let mut r = ReorderBuffer::new(0, 1024);
        let mut b = WeightBatcher::new(5, 100, Durability::None);
        let mut emitted = Vec::new();

        test_utils::insert_range_expect_empty(&mut r, 1..=9);

        test_utils::feed_ready(&mut b, r.insert(0, test_utils::mb(0, 1)), &mut emitted);
        for h in 1..=9 { test_utils::feed_ready(&mut b, r.insert(h, test_utils::mb(h, 1)), &mut emitted); }
        test_utils::flush_collect(&mut b, &mut emitted);

        assert_eq!(emitted, test_utils::range_vec(0, 9));
        assert_eq!(emitted.len(), 10);
    }

    #[test]
    fn too_low_are_dropped_and_counted() {
        let mut r = ReorderBuffer::new(200, 64);
        let before = r.too_low_count();

        // Below lower bound → dropped and not retained.
        assert!(r.insert(180, test_utils::mb(180, 1)).is_empty());
        assert!(r.insert(199, test_utils::mb(199, 1)).is_empty());
        assert_eq!(r.too_low_count(), before + 2);
        assert_eq!(r.pending_len(), 0, "too-low items must not be retained");

        // Normal flow still works.
        let mut b = WeightBatcher::new(3, 10, Durability::None);
        let mut emitted = Vec::new();
        for h in 200..=205 { test_utils::feed_ready(&mut b, r.insert(h, test_utils::mb(h, 1)), &mut emitted); }
        test_utils::flush_collect(&mut b, &mut emitted);
        assert_eq!(emitted, test_utils::range_vec(200, 205));
    }

    #[test]
    fn immediate_mode_with_reorder_integration() {
        let mut r = ReorderBuffer::new(100, 1024);
        let mut b = WeightBatcher::new(9999, 9999, Durability::Immediate);
        let mut emitted = Vec::new();

        for h in [102u32, 100, 101, 103] {
            test_utils::feed_ready(&mut b, r.insert(h, test_utils::mb(h, 1)), &mut emitted);
        }
        assert_eq!(emitted, vec![100, 101, 102, 103]);
    }
}
