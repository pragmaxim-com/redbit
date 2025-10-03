use std::marker::PhantomData;
use redb::Value;

pub trait CopyOwnedValue: Value + Copy {
    type Unit: Copy + Send + 'static;

    /// Convert from a borrowed view to the copyable unit (no move).
    fn to_unit_ref<'a>(v: &Self::SelfType<'a>) -> Self::Unit
    where
        Self: 'a;

    fn as_value_from_unit<'a>(u: &'a Self::Unit) -> Self::SelfType<'a>
    where
        Self: 'a;

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
