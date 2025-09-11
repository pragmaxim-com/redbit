use crate::AppError;
use redb::*;
use redb::{Key, Table, WriteTransaction};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;

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

pub struct DictTable<'txn, K: Key + 'static, V: Key + 'static> {
    dict_index: MultimapTable<'txn, K, K>,
    by_dict_pk: Table<'txn, K, V>,
    to_dict_pk: Table<'txn, V, K>,
    dict_pk_by_id: Table<'txn, K, K>,
}

impl<'txn, K: Key + 'static, V: Key + 'static> DictTable<'txn, K, V> {
    pub fn new(write_tx: &'txn WriteTransaction, dict_index_def: MultimapTableDefinition<K, K>, by_dict_pk_def: TableDefinition<K, V>, to_dict_pk_def: TableDefinition<V, K>, dict_pk_by_id_def: TableDefinition<K, K>) -> Result<Self, AppError> {
        Ok(Self {
            dict_index: write_tx.open_multimap_table(dict_index_def)?,
            by_dict_pk: write_tx.open_table(by_dict_pk_def)?,
            to_dict_pk: write_tx.open_table(to_dict_pk_def)?,
            dict_pk_by_id: write_tx.open_table(dict_pk_by_id_def)?,
        })
    }

    pub fn dict_insert<'k, 'v>(&mut self, key: impl Borrow<K::SelfType<'k>>, value: impl Borrow<V::SelfType<'v>>) -> Result<()>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        let val_ref: &V::SelfType<'v> = value.borrow();

        if let Some(birth_id_guard) = self.to_dict_pk.get(val_ref)? {
            let birth_id = birth_id_guard.value();
            self.dict_pk_by_id.insert(key_ref, &birth_id)?;
            self.dict_index.insert(birth_id, key_ref)?;
        } else {
            self.to_dict_pk.insert(val_ref, key_ref)?;
            self.by_dict_pk.insert(key_ref, val_ref)?;
            self.dict_pk_by_id.insert(key_ref, key_ref)?;
            self.dict_index.insert(key_ref, key_ref)?;
        }
        Ok(())
    }

    pub fn dict_delete<'k>(&mut self, key: impl Borrow<K::SelfType<'k>>) -> Result<bool, AppError>  {
        let key_ref: &K::SelfType<'k> = key.borrow();
        if let Some(birth_guard) = self.dict_pk_by_id.remove(key_ref)? {
            let birth_id = birth_guard.value();
            let was_removed = self.dict_index.remove(&birth_id, key_ref)?;
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

pub enum IndexCommand<K: Key + Send + 'static, V: Key + Send + 'static> {
    Insert(K, V),
    Remove(K, Sender<Result<bool, AppError>>), // ✅ send back signal when done
    Flush,
}

pub struct DictTableWriter<K: Key + Send + 'static, V: Key + Send + 'static> {
    topic: Sender<IndexCommand<K, V>>,
    handle: JoinHandle<()>,
}

impl<K: Key + Send + 'static + Borrow<K::SelfType<'static>>, V: Key + Send + 'static + Borrow<V::SelfType<'static>>> DictTableWriter<K, V> {
    pub fn new<F>(dict_db: Arc<Database>, open_ctx: F) -> Result<Self, AppError>
    where
        F: for<'txn> Fn(&'txn WriteTransaction) -> Result<DictTable<'txn, K, V>, AppError> + Send + 'static,
    {
        let (topic, receiver) = channel::<IndexCommand<K, V>>();
        let (ready_tx, ready_rx) = channel::<Result<(), AppError>>();

        let handle = thread::spawn(move || {
            let tx = match dict_db.begin_write() {
                Ok(tx) => tx,
                Err(e) => {
                    let _ = ready_tx.send(Err(AppError::from(e)));
                    return;
                }
            };

            match open_ctx(&tx) {
                Ok(mut table) => {
                    let _ = ready_tx.send(Ok(()));
                    loop {
                        match receiver.recv() {
                            Ok(IndexCommand::Insert(k, v)) => {
                                let _ = table.dict_insert(k, v);
                            }
                            Ok(IndexCommand::Remove(k, ack)) => {
                                let result = table.dict_delete(k);
                                let _ = ack.send(result);
                            }
                            Ok(IndexCommand::Flush) | Err(_) => break,
                        }
                    }
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
            tx.commit().expect("commit worker tx");
        });

        // wait until the worker tells us it opened the table successfully
        ready_rx.recv()??;
        Ok(Self { topic, handle })
    }

    pub fn dict_insert(&self, key: K, value: V) {
        self.topic.send(IndexCommand::Insert(key, value)).unwrap();
    }

    pub fn dict_delete(&self, key: K) -> Result<bool, AppError> {
        let (ack_tx, ack_rx) = channel::<Result<bool, AppError>>();
        self.topic.send(IndexCommand::Remove(key, ack_tx)).unwrap();
        let result = ack_rx.recv()?;
        result
    }

    pub fn flush(self) -> Result<(), AppError> {
        self.topic.send(IndexCommand::Flush).unwrap();
        self.handle.join().map_err(|_| AppError::Custom("Write table j\
        oin failed".to_string()))
    }
}

