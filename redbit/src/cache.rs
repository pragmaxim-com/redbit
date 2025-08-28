/*use std::{any::TypeId, hash::Hash, num::NonZeroUsize};

// ---------- Typed cache token (like TableDefinition) ----------
pub struct CacheDef<K, V> {
    pub name: &'static str,
    pub capacity: NonZeroUsize,
    _marker: std::marker::PhantomData<(K, V)>,
}
impl<K, V> CacheDef<K, V> {
    pub const fn new(name: &'static str, capacity: NonZeroUsize) -> Self {
        Self { name, capacity, _marker: std::marker::PhantomData }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct CacheKey {
    name: &'static str,
    type_id: TypeId,
}
impl CacheKey {
    fn of<K: 'static, V: 'static>(name: &'static str) -> Self {
        Self { name, type_id: TypeId::of::<(K, V)>() }
    }
}
*/