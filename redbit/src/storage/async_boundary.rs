use crate::DbKey;
use redb::Value;
use std::marker::PhantomData;

pub struct ValueOwned<V: DbKey> {
    unit: V::Unit,
    _pd:  PhantomData<V>,
}

impl<V: DbKey> Clone for ValueOwned<V> {
    #[inline] fn clone(&self) -> Self { *self }
}
impl<V: DbKey> Copy for ValueOwned<V> {}

impl<V: DbKey> ValueOwned<V> {
    #[inline]
    pub fn from_guard(g: redb::AccessGuard<'_, V>) -> Self {
        let v = g.value();
        let u = V::to_unit_ref(&v);
        Self { unit: u, _pd: PhantomData }
    }

    #[inline]
    pub fn from_value<'a>(v: V::SelfType<'a>) -> Self
    where
        V: 'a,
    {
        let u = V::to_unit_ref(&v);
        Self { unit: u, _pd: PhantomData }
    }

    #[inline]
    pub fn from_unit(u: V::Unit) -> Self {
        Self { unit: u, _pd: PhantomData }
    }

    #[inline]
    pub fn as_value<'a>(&'a self) -> V::SelfType<'a>
    where
        V: 'a,
    {
        V::as_value_from_unit(&self.unit)
    }

    #[inline]
    pub fn into_unit(self) -> V::Unit { self.unit }

}

pub struct ValueBuf<V: Value> {
    pub buf: Vec<u8>,
    _pd: PhantomData<V>,
}

impl<V: Value> ValueBuf<V> {
    pub fn new(buf: Vec<u8>) -> Self { Self { buf, _pd: PhantomData } }
    pub fn as_value(&self) -> V::SelfType<'_> { V::from_bytes(&self.buf) }
    pub fn as_bytes(&self) -> &[u8] { &self.buf }
}
