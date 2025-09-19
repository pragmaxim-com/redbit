use crate::AppError;
use redb::*;
use redb::Key;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ReadOnlyDictTable<K: Key + 'static, V: Key + 'static> {
    dict_pk_to_ids: ReadOnlyMultimapTable<K, K>,
    value_by_dict_pk: ReadOnlyTable<K, V>,
    value_to_dict_pk: ReadOnlyTable<V, K>,
    dict_pk_by_id: ReadOnlyTable<K, K>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyDictTable<K, V> {
    pub fn new(dict_db: Arc<Database>, dict_pk_to_ids_def: MultimapTableDefinition<K, K>, value_by_dict_pk_def: TableDefinition<K, V>, value_to_dict_pk_def: TableDefinition<V, K>, dict_pk_by_id_def: TableDefinition<K, K>) -> Result<Self, AppError> {
        let dict_tx = dict_db.begin_read()?;
        Ok(Self {
            dict_pk_to_ids: dict_tx.open_multimap_table(dict_pk_to_ids_def)?,
            value_by_dict_pk: dict_tx.open_table(value_by_dict_pk_def)?,
            value_to_dict_pk: dict_tx.open_table(value_to_dict_pk_def)?,
            dict_pk_by_id: dict_tx.open_table(dict_pk_by_id_def)?,
        })
    }

    pub fn get_value<'k>(&self, key: impl Borrow<K::SelfType<'k>>,) -> Result<Option<AccessGuard<'_, V>>> {
        let birth_guard_opt = self.dict_pk_by_id.get(key.borrow())?;
        match birth_guard_opt {
            Some(birth_guard) => {
                let birth_id = birth_guard.value();
                let val_guard = self.value_by_dict_pk.get(birth_id)?;
                match val_guard {
                    Some(vg) => Ok(Some(vg)),
                    None => Ok(None),
                }
            },
            None => Ok(None),
        }

    }
    pub fn get_keys<'v>(&self, val: impl Borrow<V::SelfType<'v>>) -> Result<Option<MultimapValue<'static, K>>> {
        let birth_guard = self.value_to_dict_pk.get(val.borrow())?;
        match birth_guard {
            Some(g) => {
                let birth_id = g.value();
                let value = self.dict_pk_to_ids.get(&birth_id)?;
                Ok(Some(value))
            },
            None => Ok(None),
        }
    }

    pub fn stats(&self) -> Result<HashMap<String, TableStats>> {
        let mut stats = HashMap::new();
        stats.insert("dict_pk_to_ids".to_string(), self.dict_pk_to_ids.stats()?);
        stats.insert("value_by_dict_pk".to_string(), self.value_by_dict_pk.stats()?);
        stats.insert("value_to_dict_pk".to_string(), self.value_to_dict_pk.stats()?);
        stats.insert("dict_pk_by_id".to_string(), self.dict_pk_by_id.stats()?);
        Ok(stats)
    }
}
