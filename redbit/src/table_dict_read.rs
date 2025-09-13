use crate::AppError;
use redb::*;
use redb::Key;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ReadOnlyDictTable<K: Key + 'static, V: Key + 'static> {
    dict_index: ReadOnlyMultimapTable<K, K>,
    by_dict_pk: ReadOnlyTable<K, V>,
    to_dict_pk: ReadOnlyTable<V, K>,
    dict_pk_by_id: ReadOnlyTable<K, K>,
}

impl<K: Key + 'static, V: Key + 'static> ReadOnlyDictTable<K, V> {
    pub fn new(dict_db: Arc<Database>, dict_index_def: MultimapTableDefinition<K, K>, by_dict_pk_def: TableDefinition<K, V>, to_dict_pk_def: TableDefinition<V, K>, dict_pk_by_id_def: TableDefinition<K, K>) -> Result<Self, AppError> {
        let dict_tx = dict_db.begin_read()?;
        Ok(Self {
            dict_index: dict_tx.open_multimap_table(dict_index_def)?,
            by_dict_pk: dict_tx.open_table(by_dict_pk_def)?,
            to_dict_pk: dict_tx.open_table(to_dict_pk_def)?,
            dict_pk_by_id: dict_tx.open_table(dict_pk_by_id_def)?,
        })
    }

    pub fn get_value<'k>(&self, key: impl Borrow<K::SelfType<'k>>,) -> Result<Option<AccessGuard<'_, V>>> {
        let birth_guard_opt = self.dict_pk_by_id.get(key.borrow())?;
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
    pub fn get_keys<'v>(&self, val: impl Borrow<V::SelfType<'v>>) -> Result<Option<MultimapValue<'static, K>>> {
        let birth_guard = self.to_dict_pk.get(val.borrow())?;
        match birth_guard {
            Some(g) => {
                let birth_id = g.value();
                let value = self.dict_index.get(&birth_id)?;
                Ok(Some(value))
            },
            None => Ok(None),
        }
    }

    pub fn stats(&self) -> Result<HashMap<String, TableStats>> {
        let mut stats = HashMap::new();
        stats.insert("dict_index".to_string(), self.dict_index.stats()?);
        stats.insert("by_dict_pk".to_string(), self.by_dict_pk.stats()?);
        stats.insert("to_dict_pk".to_string(), self.to_dict_pk.stats()?);
        stats.insert("dict_pk_by_id".to_string(), self.dict_pk_by_id.stats()?);
        Ok(stats)
    }
}
