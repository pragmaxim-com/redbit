use crate::AppError;
use redb::{Key, MultimapValue};
use std::borrow::Borrow;
use std::cmp::Ordering;

pub fn assert_sorted<T, I>(items: &[T], label: &str, mut extract: impl FnMut(&T) -> &I)
where
    I: Key + Borrow<I::SelfType<'static>> + 'static,
{
    for w in items.windows(2) {
        let ia = extract(&w[0]);
        let ib = extract(&w[1]);
        let ord = <I as Key>::compare(
            <I as redb::Value>::as_bytes(ia.borrow()).as_ref(),
            <I as redb::Value>::as_bytes(ib.borrow()).as_ref(),
        );
        assert!(matches!(ord, Ordering::Less | Ordering::Equal), "{} must be sorted by key", label);
    }
}

pub fn collect_multimap_value<'a, V: Key + 'a>(mut mmv: MultimapValue<'a, V>) -> Result<Vec<V>, AppError>
where
        for<'b> <V as redb::Value>::SelfType<'b>: ToOwned<Owned = V>,
{
    let mut results = Vec::new();
    while let Some(item_res) = mmv.next() {
        let guard = item_res?;
        results.push(guard.value().to_owned());
    }
    Ok(results)
}

pub fn inc_le(bytes: &mut [u8]) {
    for b in bytes.iter_mut() {
        if *b != 0xFF {
            *b = b.wrapping_add(1);
            return;
        }
        *b = 0;
    }
}
