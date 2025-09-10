use redb::*;

pub struct ReadOnlyDictTable<K: Key + 'static, V: Key + 'static> {
    dict_index: ReadOnlyMultimapTable<K, K>,
    by_dict_pk: ReadOnlyTable<K, V>,
    to_dict_pk: ReadOnlyTable<V, K>,
    dict_pk_by_id: ReadOnlyTable<K, K>,
}

pub struct DictTable<'txn, K: Key + 'static, V: Key + 'static> {
    dict_index: MultimapTable<'txn, K, K>,
    by_dict_pk: Table<'txn, K, V>,
    to_dict_pk: Table<'txn, V, K>,
    dict_pk_by_id: Table<'txn, K, K>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyDictTable<K, V> {
    pub fn new(dict_index: ReadOnlyMultimapTable<K, K>, by_dict_pk: ReadOnlyTable<K, V>, to_dict_pk: ReadOnlyTable<V, K>, dict_pk_by_id: ReadOnlyTable<K, K>) -> Self {
        Self { dict_index, by_dict_pk, to_dict_pk, dict_pk_by_id }
    }

    pub fn get_value(&self, key: K::SelfType<'_>) -> Result<Option<AccessGuard<'_, V>>> {
        let birth_guard_opt = self.dict_pk_by_id.get(key)?;
        match birth_guard_opt {
            Some(birth_guard) => {
                let birth_id = birth_guard.value();
                let val_guard = self.by_dict_pk.get(birth_id)?;
                match val_guard {
                    Some(vg) => Ok(Some(vg)),
                    None => Ok(None),
                }
            },
            None => Ok(None),
        }

    }
    pub fn get_keys(&self, val: V::SelfType<'_>) -> Result<Option<MultimapValue<'static, K>>> {
        let birth_guard = self.to_dict_pk.get(val)?;
        match birth_guard {
            Some(g) => {
                let birth_id = g.value();
                let value = self.dict_index.get(&birth_id)?;
                Ok(Some(value))
            },
            None => Ok(None),
        }
    }
}

impl<'txn, K: Key + 'static, V: Key + 'static> DictTable<'txn, K, V> {
    pub fn new(dict_index: MultimapTable<'txn, K, K>, by_dict_pk: Table<'txn, K, V>, to_dict_pk: Table<'txn, V, K>, dict_pk_by_id: Table<'txn, K, K>) -> Self {
        Self { dict_index, by_dict_pk, to_dict_pk, dict_pk_by_id }
    }

    pub fn dict_insert(&mut self, key: K::SelfType<'_>, value: V::SelfType<'_>) -> Result<()>  {
        if let Some(birth_id_guard) = self.to_dict_pk.get(&value)? {
            let birth_id = birth_id_guard.value();
            self.dict_pk_by_id.insert(&key, &birth_id)?;
            self.dict_index.insert(&birth_id, &key)?;
        } else {
            self.to_dict_pk.insert(&value, &key)?;
            self.by_dict_pk.insert(&key, &value)?;
            self.dict_pk_by_id.insert(&key, &key)?;
            self.dict_index.insert(&key, &key)?;
        }
        Ok(())
    }

    pub fn dict_delete(&mut self, key: K::SelfType<'_>) -> Result<bool>  {
        if let Some(birth_guard) = self.dict_pk_by_id.remove(&key)? {
            let birth_id = birth_guard.value();
            let was_removed = self.dict_index.remove(&birth_id, key)?;
            if self.dict_index.get(&birth_id)?.is_empty() {
                if let Some(value_guard) = self.by_dict_pk.remove(&birth_id)? {
                    let value = value_guard.value();
                    self.to_dict_pk.remove(&value)?;
                }
            }
            Ok(was_removed)
        } else {
            Ok(false)
        }
    }
}
