use std::marker::PhantomData;
use redb::Value;

pub trait CopyOwnedValue: Value + Copy {
    type Unit: Copy + Send + 'static;

    fn to_unit<'a>(v: Self::SelfType<'a>) -> Self::Unit
    where
        Self: 'a;

    fn from_unit<'a>(u: Self::Unit) -> Self::SelfType<'a>
    where
        Self: 'a;

}

pub struct ValueOwned<V: CopyOwnedValue> {
    unit: V::Unit,
    _pd:  PhantomData<V>,
}

impl<V: CopyOwnedValue> Clone for ValueOwned<V> {
    #[inline] fn clone(&self) -> Self { *self }
}
impl<V: CopyOwnedValue> Copy for ValueOwned<V> {}

impl<V: CopyOwnedValue> ValueOwned<V> {
    #[inline]
    pub fn from_guard(g: redb::AccessGuard<'_, V>) -> Self {
        Self { unit: V::to_unit(g.value()), _pd: PhantomData }
    }

    #[inline]
    pub fn from_value<'a>(v: V::SelfType<'a>) -> Self
    where
        V: 'a,
    {
        Self { unit: V::to_unit(v), _pd: PhantomData }
    }

    #[inline]
    pub fn as_value<'a>(&'a self) -> V::SelfType<'a>
    where
        V: 'a,
    {
        V::from_unit(self.unit)
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
