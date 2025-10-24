use std::collections::hash_map::Entry;
use crate::api::Height;
use std::collections::HashMap;
use redbit::warn;

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
    max_seen: Option<Height>,

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
            max_seen: None,
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
            warn!("ReorderBuffer: dropped item for height {} below next_expected {}", height, self.next_height);
            return Vec::new();
        }
        match self.max_seen {
            None => self.max_seen = Some(height),
            Some(ms) if height > ms => self.max_seen = Some(height),
            _ => {}
        }
        // First-come-wins; ignore duplicates to avoid double-release.
        match self.pending.entry(height) {
            Entry::Vacant(v) => { v.insert(block); }
            Entry::Occupied(_) => { warn!("ReorderBuffer: duplicate for height {}", height);}
        }
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

    /// Current gap (next_expected..=max_seen), if any.
    pub fn gap_span(&self) -> Option<(Height, Height)> {
        match self.max_seen {
            None => None,
            Some(ms) if ms >= self.next_height => Some((self.next_height, ms)),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn next_expected(&self) -> Height { self.next_height }

    #[allow(dead_code)]
    pub(crate) fn max_seen(&self) -> Option<Height> { self.max_seen }

    #[allow(dead_code)]
    pub(crate) fn too_low_count(&self) -> u32 { self.dropped_too_low }
}

#[cfg(test)]
mod tests {
    use crate::reorder_buffer::ReorderBuffer;
    use crate::test_utils;

    #[test]
    fn gap_span_starting_at_zero_should_be_none() {
        let rb = ReorderBuffer::<()>::new(0, 1024);
        // before the fix, gap_span() would have returned Some((0,0))
        assert!(rb.gap_span().is_none(), "gap_span must be None before any heights seen");
    }

    #[test]
    fn diagnostics_show_gap_span() {
        let mut r = ReorderBuffer::new(50, 4);
        test_utils::insert_range_expect_empty(&mut r, [52u32, 53, 60, 61, 62]);

        assert_eq!(r.next_expected(), 50u32);
        assert_eq!(r.max_seen(), Some(62u32));

        match r.gap_span() {
            Some((lo, hi)) => { assert_eq!(lo, 50u32); assert_eq!(hi, 62u32); }
            None => panic!("expected a gap"),
        }
        assert!(r.is_saturated(), "soft hint reached at pending_len >= 4");
        assert_eq!(r.pending_len(), 5);
    }

    #[test]
    fn duplicates_ignored() {
        let mut r = ReorderBuffer::new(10, 1024);
        assert!(r.insert(12, test_utils::mb(12, 1)).is_empty());
        assert!(r.insert(12, test_utils::mb(12, 1)).is_empty());

        let out1 = r.insert(10, test_utils::mb(10, 1));
        assert_eq!(test_utils::heights(&out1), vec![10]);

        let out2 = r.insert(11, test_utils::mb(11, 1));
        assert_eq!(test_utils::heights(&out2), vec![11, 12]);
    }
}
