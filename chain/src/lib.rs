pub mod api;
pub mod settings;
pub mod syncer;
pub mod monitor;
pub mod scheduler;
pub mod combine;
pub mod launcher;
pub mod task;
pub mod weight_batcher;
pub mod stats;
pub mod chain_config;
pub mod size_batcher;
pub mod block_stream;
mod reorder_buffer;

pub use api::{BlockHeaderLike, SizeLike, BlockLike, BlockChainLike, ChainError};

#[cfg(test)]
pub mod test_utils {
    use crate::reorder_buffer::ReorderBuffer;
    use crate::weight_batcher::WeightBatcher;

    #[derive(Clone, Debug)]
    pub(crate) struct MockHeader { h: u32, w: usize }
    #[derive(Clone, Debug)]
    pub(crate) struct MockBlock(MockHeader);

    impl MockBlock {
        pub(crate) fn header(&self) -> &MockHeader { &self.0 }
    }
    impl MockHeader {
        pub(crate) fn height(&self) -> u32 { self.h }
        pub(crate) fn weight(&self) -> usize { self.w }
    }

    pub(crate) fn mb(h: u32, w: usize) -> MockBlock { MockBlock(MockHeader { h, w }) }

    /// Push one item into the batcher; if a batch is produced, append its heights.
    pub(crate) fn push_collect_heights(
        b: &mut WeightBatcher<MockBlock>,
        item: MockBlock,
        out: &mut Vec<u32>,
    ) {
        if let Some(batch) = b.push_with(item, |x| x.header().weight()) {
            out.extend(heights(&batch));
        }
    }

    /// Feed a sequence of ready blocks into the batcher and append produced heights.
    pub(crate) fn feed_ready<I: IntoIterator<Item = MockBlock>>(
        b: &mut WeightBatcher<MockBlock>,
        ready: I,
        out: &mut Vec<u32>,
    ) {
        for blk in ready { push_collect_heights(b, blk, out); }
    }

    /// Flush the batcher and append produced heights (if any).
    pub(crate) fn flush_collect(b: &mut WeightBatcher<MockBlock>, out: &mut Vec<u32>) {
        if let Some(tail) = b.flush() { out.extend(heights(&tail)); }
    }

    /// Collect heights from a slice/vec of blocks.
    pub(crate) fn heights(blocks: &[MockBlock]) -> Vec<u32> {
        blocks.iter().map(|b| b.header().height()).collect()
    }

    /// Convenience for inclusive range -> Vec<u32>
    pub(crate) fn range_vec(lo: u32, hi: u32) -> Vec<u32> {
        (lo..=hi).collect()
    }

    /// Insert a range and assert that no items are emitted (all inserts return empty).
    pub(crate) fn insert_range_expect_empty(
        r: &mut ReorderBuffer<MockBlock>,
        rng: impl IntoIterator<Item = u32>,
    ) {
        for h in rng {
            assert!(r.insert(h, mb(h, 1)).is_empty(), "expected no emission for height {}", h);
        }
    }

}