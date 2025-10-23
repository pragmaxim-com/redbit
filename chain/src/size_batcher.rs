use crate::SizeLike;

/// Anything that can be batched by byte-size.
pub trait BatchSized {
    fn batch_size(&self) -> usize;
}

pub struct SizeBatcher<T: SizeLike> {
    byte_limit: usize,
    buf: Vec<T>,
    bytes: usize,
    immediate: bool,
}

impl<T: SizeLike> SizeBatcher<T> {
    /// Optionally provide an initial reserve to avoid first-grow cost.
    pub fn new(byte_limit: usize, immediate: bool) -> Self {
        Self { byte_limit, buf: Vec::new(), bytes: 0, immediate }
    }

    pub fn from_kb(kb_limit: usize, immediate: bool) -> Self {
        Self::new(kb_limit * 1024, immediate)
    }

    #[inline]
    pub fn push(&mut self, item: T) -> Option<Vec<T>> {
        if self.immediate {
            return Some(vec![item]);
        }

        let item_size = item.size();
        let projected = self.bytes.saturating_add(item_size);

        if projected > self.byte_limit {
            return if self.buf.is_empty() {
                Some(vec![item])
            } else {
                let old_cap = self.buf.capacity();
                let old_buf = std::mem::replace(&mut self.buf, Vec::with_capacity(old_cap));
                self.bytes = item_size;
                self.buf.push(item);
                Some(old_buf)
            }
        }

        // safe to append
        self.bytes = projected;
        self.buf.push(item);
        None
    }

    #[inline]
    pub fn take_all(&mut self) -> Option<Vec<T>> {
        if self.immediate || self.buf.is_empty() { return None; }
        let old_cap = self.buf.capacity();
        let out = std::mem::replace(&mut self.buf, Vec::with_capacity(old_cap));
        self.bytes = 0;
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bytes(Vec<u8>);
    impl SizeLike for Bytes {
        fn size(&self) -> usize {
            self.0.len()
        }
    }
    fn b(n: usize) -> Bytes { Bytes(vec![0; n]) }

    #[test]
    fn immediate_flushes_each_item() {
        let mut s = SizeBatcher::new(100, true);
        assert_eq!(s.push(b(10)).unwrap().len(), 1);
        assert_eq!(s.push(b(20)).unwrap().len(), 1);
        assert!(s.take_all().is_none());
    }

    #[test]
    fn flushes_before_adding_overflowing_item() {
        // limit = 50, sequence 10,20,25
        // after 10,20 -> bytes = 30, push 25 would make 55 > 50
        // So we flush [10,20] and keep 25 in the new buffer
        let mut s = SizeBatcher::new(50, false);
        assert!(s.push(b(10)).is_none());
        assert!(s.push(b(20)).is_none());
        let batch = s.push(b(25)).unwrap(); // returns previous batch [10,20]
        assert_eq!(batch.len(), 2);
        // now remaining buffer contains the 25
        let tail = s.take_all().unwrap();
        assert_eq!(tail.len(), 1);
    }

    #[test]
    fn single_large_item_returns_immediately() {
        // item size 120 > limit 100, buffer empty -> returned as single batch
        let mut s = SizeBatcher::new(100, false);
        let batch = s.push(b(120)).unwrap();
        assert_eq!(batch.len(), 1);
        assert!(s.take_all().is_none());
    }

    #[test]
    fn flushes_tail() {
        let mut s = SizeBatcher::new(100, false);
        assert!(s.push(b(40)).is_none());
        assert!(s.push(b(50)).is_none());
        let tail = s.take_all().unwrap();
        assert_eq!(tail.len(), 2);
    }
}
