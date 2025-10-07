use redb::{Key, Value};
use std::borrow::Borrow;
use xxhash_rust::xxh3::{xxh3_64};

/*
use wyhash::wyhash;

#[derive(Clone)]
pub struct WyHashPartitioner { n: usize, seed: u64 }
impl WyHashPartitioner {
    #[inline] pub fn new(n: usize, seed: u64) -> Self { assert!(n>0); Self { n, seed } }
    #[inline] pub fn partition_bytes(&self, bytes: &[u8]) -> usize {
        if self.n == 1 { return 0; }
        let h = wyhash(bytes, self.seed);
        (h % (self.n as u64)) as usize
    }
}
impl<V> ValuePartitioner<V> for WyHashPartitioner
where V: Key + 'static + Borrow<V::SelfType<'static>> {
    #[inline]
    fn partition_value<'v>(&self, value: impl Borrow<V::SelfType<'v>>) -> usize {
        if self.n == 1 { return 0; }
        let bytes_view = <V as redb::Value>::as_bytes(value.borrow());
        self.partition_bytes(bytes_view.as_ref())
    }
}
*/

#[derive(Clone)]
pub enum Partitioning<KP, VP> {
    ByKey(KP),
    ByValue(VP),
}

impl Partitioning<BytesPartitioner, Xxh3Partitioner> {
    pub fn by_key(shards: usize) -> Partitioning<BytesPartitioner, Xxh3Partitioner> {
        Partitioning::ByKey(BytesPartitioner::new(shards))
    }
    pub fn by_value(shards: usize) -> Partitioning<BytesPartitioner, Xxh3Partitioner> {
        Partitioning::ByValue(Xxh3Partitioner::new(shards))
    }
}

pub trait ValuePartitioner<V: Key + 'static + Borrow<V::SelfType<'static>>> {
    fn partition_value<'v>(&self, value: impl Borrow<V::SelfType<'v>>) -> usize;
}

/// Fast, deterministic partitioner for arbitrary byte values.
/// O(len), no allocations; stable across runs with a fixed `seed`.
#[derive(Clone)]
pub struct Xxh3Partitioner(usize);

impl Xxh3Partitioner {
    pub fn new(n: usize) -> Self {
        assert!(n > 0, "shard count must be > 0");
        Self(n)
    }

    #[inline]
    pub fn partition_bytes(&self, bytes: &[u8]) -> usize {
        (xxh3_64(bytes) % (self.0 as u64)) as usize
    }
}

impl<V> ValuePartitioner<V> for Xxh3Partitioner
where
    V: Key + 'static + Borrow<V::SelfType<'static>>,
{
    #[inline]
    fn partition_value<'v>(&self, value: impl Borrow<V::SelfType<'v>>) -> usize {
        let bytes_view = <V as Value>::as_bytes(value.borrow());
        self.partition_bytes(bytes_view.as_ref())
    }
}

// ---------- Partitioning trait ----------
pub trait KeyPartitioner<K: Key + 'static + Borrow<K::SelfType<'static>>> {
    fn partition_key<'k>(&self, key: impl Borrow<K::SelfType<'k>>) -> usize;
}

// ---------- Struct adapter (kept) ----------
#[derive(Clone)]
pub struct BytesPartitioner(usize);

impl BytesPartitioner {
    pub fn new(n: usize) -> Self {
        assert!(n > 0, "shard count must be > 0");
        Self(n)
    }

    #[inline]
    pub fn partition_bytes(&self, bytes: &[u8]) -> usize {
        partition_bytes_le(self.0, bytes)
    }
}

impl<K> KeyPartitioner<K> for BytesPartitioner
where
    K: Key + 'static + Borrow<K::SelfType<'static>>,
{
    #[inline]
    fn partition_key<'k>(&self, key: impl Borrow<K::SelfType<'k>>) -> usize {
        partition_key_redb::<K>(self.0, key.borrow())
    }
}

// ---------- Zero-alloc functional core (shared) ----------

/// Partition a redb key given a borrow of its `SelfType<'_>`.
#[inline]
pub fn partition_key_redb<K: Key>(n: usize, key: &K::SelfType<'_>) -> usize {
    assert!(n > 0, "shard count must be > 0");
    let bytes = <K as Value>::as_bytes(key);
    partition_bytes_le(n, bytes.as_ref())
}

/// Partition directly from a contiguous little-endian byte slice.
#[inline]
pub fn partition_bytes_le(n: usize, bytes: &[u8]) -> usize {
    let m = n as u128;
    if m == 1 { return 0; }
    let mut pow = 1u128;
    let mut acc = 0u128;
    le_mod_fold_chunk(m, &mut pow, &mut acc, bytes);
    (acc % m) as usize
}

#[inline]
fn le_mod_fold_chunk(m: u128, pow: &mut u128, acc: &mut u128, chunk: &[u8]) {
    if m == 1 { return; }
    let mut p = *pow;
    let mut a = *acc;
    for &b in chunk {
        a = (a + (b as u128) * p) % m;
        p = (p * 256) % m;
    }
    *pow = p;
    *acc = a;
}

#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use super::*;
    use redb::{TypeName, Value};
    use std::borrow::Cow;
    use std::cmp::Ordering;

    #[inline]
    fn manual_reduce_le(bytes: &[u8], n: usize) -> usize {
        if n == 1 { return 0; }
        let m = n as u128;
        let mut pow = 1u128;
        let mut acc = 0u128;
        for &b in bytes {
            acc = (acc + (b as u128) * pow) % m;
            pow = (pow * 256) % m;
        }
        acc as usize
    }

    fn make_bytes(len: usize, mut seed: u64) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        seed ^= 0x9E37_79B1_85EB_CA87u64;
        for _ in 0..len {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            out.push((seed >> 24) as u8);
        }
        out
    }

    // ---- mock redb::Key types ----

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct BorrowKey(Vec<u8>);
    impl Value for BorrowKey {
        type SelfType<'a> = BorrowKey where Self: 'a;
        type AsBytes<'a> = Cow<'a, [u8]> where Self: 'a;
        fn fixed_width() -> Option<usize> { None }
        fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a> where Self: 'a { BorrowKey(data.to_vec()) }
        fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> where Self: 'b { Cow::Borrowed(&value.0) }
        fn type_name() -> TypeName { TypeName::new("BorrowKey") }
    }
    impl Key for BorrowKey { fn compare(a: &[u8], b: &[u8]) -> Ordering { a.cmp(b) } }

    // ---- tests ----

    #[test]
    fn bytes_partition_in_range_and_deterministic() {
        for &n in &[1usize, 3, 6, 8, 17, 32] {
            let p = BytesPartitioner::new(n);
            for len in [0usize, 1, 3, 8, 13, 32] {
                let b = make_bytes(len, 42);
                let s1 = p.partition_bytes(&b);
                let s2 = p.partition_bytes(&b);
                assert!(s1 < n);
                assert_eq!(s1, s2);
                assert_eq!(s1, manual_reduce_le(&b, n));
            }
        }
    }

    #[test]
    fn key_partition_accepts_borrows_and_matches_bytes() {
        for &n in &[2usize, 4, 8, 16, 31] {
            let p = BytesPartitioner::new(n);
            for len in [0usize, 2, 7, 19, 64] {
                let b = make_bytes(len, 123);

                let kb = BorrowKey(b.clone());

                // function form: pass borrow of SelfType<'_>
                assert_eq!(partition_key_redb::<BorrowKey>(n, &kb), partition_bytes_le(n, &b));

                // trait form: accepts impl Borrow<SelfType<'_>>
                assert_eq!(<BytesPartitioner as KeyPartitioner<BorrowKey>>::partition_key(&p, kb.borrow()), p.partition_bytes(&b));
            }
        }
    }

    #[test]
    fn n_equal_one_is_always_zero() {
        let n = 1usize;
        let p = BytesPartitioner::new(n);
        for len in [0usize, 1, 2, 31, 128] {
            let b = make_bytes(len, 555);
            let kb = BorrowKey(b.clone());
            assert_eq!(partition_bytes_le(n, &b), 0);
            assert_eq!(partition_key_redb::<BorrowKey>(n, &kb), 0);
            assert_eq!(p.partition_bytes(&b), 0);
            assert_eq!(<BytesPartitioner as KeyPartitioner<BorrowKey>>::partition_key(&p, kb.borrow()), 0);
        }
    }

    #[test]
    fn power_of_two_depends_only_on_low_k_bytes() {
        for &(n, k) in &[(2usize, 1u32), (4, 2), (8, 3), (16, 4)] {
            for len in [k as usize, (k as usize)+1, (k as usize)+7, 32] {
                let base = make_bytes(len, 7777);
                if base.len() < k as usize { continue; }
                let mut alt = base.clone();
                for i in (k as usize)..len { alt[i] = alt[i].wrapping_add(17); }

                assert_eq!(partition_bytes_le(n, &base), partition_bytes_le(n, &alt));

                let kb = BorrowKey(base.clone());
                let ka = BorrowKey(alt.clone());
                assert_eq!(partition_key_redb::<BorrowKey>(n, &kb), partition_key_redb::<BorrowKey>(n, &ka));
                assert_eq!(partition_key_redb::<BorrowKey>(n, &kb), partition_bytes_le(n, &base));

                let only_lowk = |bytes: &[u8]| manual_reduce_le(&bytes[..k as usize], n);
                assert_eq!(partition_bytes_le(n, &base), only_lowk(&base));
                assert_eq!(partition_bytes_le(n, &alt),  only_lowk(&alt));
            }
        }
    }

    #[test]
    fn distribution_when_lowest_byte_varies_power_of_two() {
        let n = 8usize;
        let p = BytesPartitioner::new(n);
        let mut base = make_bytes(12, 4242);
        let mut counts = [0usize; 8];
        for b0 in 0u16..256u16 {
            base[0] = (b0 as u8).wrapping_mul(7).wrapping_add(3);
            counts[p.partition_bytes(&base)] += 1;
        }
        let (&min, &max) = (counts.iter().min().unwrap(), counts.iter().max().unwrap());
        assert!(max as f64 / min as f64 <= 1.15, "imbalance {:?} (max={}, min={})", counts, max, min);
    }

    #[test]
    fn distribution_when_lowest_byte_varies_non_power_of_two() {
        let n = 6usize;
        let p = BytesPartitioner::new(n);
        let mut base = make_bytes(12, 9999);
        let mut counts = vec![0usize; n];
        for b0 in 0u16..256u16 {
            base[0] = (b0 as u8).wrapping_mul(11).wrapping_add(5);
            counts[p.partition_bytes(&base)] += 1;
        }
        let min = *counts.iter().min().unwrap();
        let max = *counts.iter().max().unwrap();
        assert!(max as f64 / min as f64 <= 1.20, "imbalance {:?} (max={}, min={})", counts, max, min);
    }

    #[test]
    fn long_keys_match_reference_for_various_n() {
        let data = make_bytes(256, 13579);
        for &n in &[3usize, 5, 7, 8, 17, 32, 64] {
            assert_eq!(partition_bytes_le(n, &data), manual_reduce_le(&data, n));
        }
        let kb = BorrowKey(data.clone());
        for &n in &[7usize, 16, 31, 64] {
            assert_eq!(partition_key_redb::<BorrowKey>(n, &kb), partition_bytes_le(n, &data));
        }
    }

    #[test]
    fn empty_bytes_are_supported() {
        let empty: [u8; 0] = [];
        for &n in &[1usize, 2, 8, 13] {
            assert_eq!(partition_bytes_le(n, &empty), manual_reduce_le(&empty, n));
        }
        let kb = BorrowKey(empty.to_vec());
        for &n in &[2usize, 8, 13] {
            assert_eq!(partition_key_redb::<BorrowKey>(n, &kb), manual_reduce_le(&empty, n));
        }
    }
}
