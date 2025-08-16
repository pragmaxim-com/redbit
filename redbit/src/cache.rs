use std::{
    any::{Any, TypeId},
    collections::HashMap,
    hash::Hash,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};
use lru::LruCache;

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

#[derive(Default)]
pub struct Caches {
    inner: Mutex<HashMap<CacheKey, Arc<dyn Any + Send + Sync>>>,
}
impl Caches {
    fn new_cache<K, V>(capacity: NonZeroUsize) -> Arc<dyn Any + Send + Sync>
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        Arc::new(Mutex::new(LruCache::<K, V>::new(capacity))) as Arc<dyn Any + Send + Sync>
    }

    pub(crate) fn get_cache<K, V>(&self, def: &'static CacheDef<K, V>) -> Arc<Mutex<LruCache<K, V>>>
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let key = CacheKey::of::<K, V>(def.name);
        let mut map = self.inner.lock().unwrap();
        let erased = map.entry(key).or_insert_with(|| Self::new_cache::<K, V>(def.capacity)).clone();
        drop(map);
        Arc::downcast::<Mutex<LruCache<K, V>>>(erased).expect(&format!("cache '{}' reused with different K/V types", def.name))
    }
}
