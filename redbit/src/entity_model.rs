use crate::error::AppError;

/// Describes how a single field writes/reads itself with minimal macro glue.
pub struct FieldSpec<E, K, RCtx, WCtx> {
    pub name: &'static str,
    pub store: fn(&WCtx, &E, bool) -> Result<(), AppError>,
    pub load: fn(&RCtx, &K) -> Result<LoadResult<E>, AppError>,
}

/// Captures the runtime layout for one entity and drives shared helpers.
pub struct EntitySpec<E, K, RCtx, WCtx> {
    pub name: &'static str,
    pub seed_with_key: fn(&K) -> E,
    pub fields: Vec<FieldSpec<E, K, RCtx, WCtx>>,
}

/// Indicates how to apply a loaded field when composing an instance.
pub enum LoadResult<E> {
    Value(Box<dyn FnOnce(E) -> E + Send + 'static>),
    Skip,
    Reject,
}

/// In-memory column used in tests to mirror store/compose roundtrips.
#[cfg(test)]
#[derive(Clone, Default)]
struct MemColumn<K, V> {
    inner: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<K, V>>>,
}

#[cfg(test)]
impl<K: Eq + std::hash::Hash + Copy, V: Clone> MemColumn<K, V> {
    fn put(&self, key: K, value: V) {
        self.inner.lock().unwrap().insert(key, value);
    }
    fn get(&self, key: &K) -> Option<V> {
        self.inner.lock().unwrap().get(key).cloned()
    }
}

/// Stores an entity by delegating to every field spec in O(n_fields).
pub fn store_entity<E, K, RCtx, WCtx>(
    spec: &EntitySpec<E, K, RCtx, WCtx>,
    ctx: &WCtx,
    entity: &E,
    is_last: bool,
) -> Result<(), AppError> {
    for field in &spec.fields {
        (field.store)(ctx, entity, is_last)?;
    }
    Ok(())
}

/// Composes an entity by folding field loaders in order until rejection.
pub fn compose_entity<E, K: Copy, RCtx, WCtx>(
    spec: &EntitySpec<E, K, RCtx, WCtx>,
    ctx: &RCtx,
    pk: K,
) -> Result<Option<E>, AppError> {
    let mut entity = (spec.seed_with_key)(&pk);
    for field in &spec.fields {
        match (field.load)(ctx, &pk)? {
            LoadResult::Value(apply) => {
                entity = apply(entity);
            }
            LoadResult::Skip => {}
            LoadResult::Reject => return Ok(None),
        }
    }
    Ok(Some(entity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug)]
    struct Sample {
        id: u32,
        a: i32,
        b: Option<String>,
    }

    impl Sample {
        fn new(id: &u32) -> Self {
            Sample { id: *id, a: 0, b: None }
        }
    }

    #[derive(Clone, Default)]
    struct WriteCtx {
        log: Arc<Mutex<Vec<String>>>,
        ints: MemColumn<u32, i32>,
        strings: MemColumn<u32, String>,
    }

    #[derive(Clone, Default)]
    struct ReadCtx {
        ints: MemColumn<u32, i32>,
        strings: MemColumn<u32, String>,
        rejects: Arc<Mutex<Vec<u32>>>,
    }

    impl ReadCtx {
        fn reject(&self, pk: u32) {
            self.rejects.lock().unwrap().push(pk);
        }
        fn put_int(&self, pk: u32, val: i32) { self.ints.put(pk, val); }
        fn put_str(&self, pk: u32, val: &str) { self.strings.put(pk, val.to_string()); }
        fn should_reject(&self, pk: u32) -> bool {
            self.rejects.lock().unwrap().contains(&pk)
        }
    }

    impl WriteCtx {
        /// Shares backing storage with a read context so store→compose reflects persisted values.
        fn shared_from(read: &ReadCtx) -> Self {
            WriteCtx { log: Arc::new(Mutex::new(Vec::new())), ints: read.ints.clone(), strings: read.strings.clone() }
        }
    }

    fn sample_spec() -> EntitySpec<Sample, u32, ReadCtx, WriteCtx> {
        let fields = vec![
            FieldSpec {
                name: "a",
                store: |wctx: &WriteCtx, entity: &Sample, is_last: bool| {
                    wctx.log.lock().unwrap().push(format!("store a {} last={}", entity.a, is_last));
                    wctx.ints.put(entity.id, entity.a);
                    Ok(())
                },
                load: move |rc: &ReadCtx, pk: &u32| {
                    if rc.should_reject(*pk) {
                        return Ok(LoadResult::Reject);
                    }
                    let v = rc.ints.get(pk).unwrap_or_default();
                    Ok(LoadResult::Value(Box::new(move |mut e: Sample| {
                        e.a = v;
                        e
                    })))
                },
            },
            FieldSpec {
                name: "b",
                store: |wctx: &WriteCtx, entity: &Sample, _is_last: bool| {
                    if let Some(text) = &entity.b {
                        wctx.strings.put(entity.id, text.clone());
                    }
                    Ok(())
                },
                load: move |rc: &ReadCtx, pk: &u32| {
                    match rc.strings.get(pk) {
                        None => Ok(LoadResult::Skip),
                        Some(text) => Ok(LoadResult::Value(Box::new(move |mut e: Sample| {
                            e.b = Some(text);
                            e
                        }))),
                    }
                },
            },
        ];
        EntitySpec {
            name: "Sample",
            seed_with_key: Sample::new,
            fields,
        }
    }

    #[test]
    fn store_entity_runs_all_fields() {
        let read_ctx = ReadCtx::default();
        let write_ctx = WriteCtx::shared_from(&read_ctx);
        let spec = sample_spec();
        let entity = Sample { id: 1, a: 7, b: Some("x".into()) };
        store_entity(&spec, &write_ctx, &entity, true).unwrap();
        let log = write_ctx.log.lock().unwrap().clone();
        assert_eq!(log, vec!["store a 7 last=true"]);
        assert_eq!(read_ctx.ints.get(&1), Some(7));
        assert_eq!(read_ctx.strings.get(&1).as_deref(), Some("x"));
    }

    #[test]
    fn compose_entity_builds_or_rejects() {
        let read_ctx = ReadCtx::default();
        read_ctx.put_int(1, 5);
        read_ctx.put_str(1, "hello");
        read_ctx.put_int(2, 9);
        read_ctx.reject(2);
        let spec = sample_spec();

        let entity = compose_entity(&spec, &read_ctx, 1).unwrap().unwrap();
        assert_eq!(entity.a, 5);
        assert_eq!(entity.b.as_deref(), Some("hello"));

        let rejected = compose_entity(&spec, &read_ctx, 2).unwrap();
        assert!(rejected.is_none());
    }

    #[test]
    fn store_and_compose_roundtrip() {
        let read_ctx = ReadCtx::default();
        let write_ctx = WriteCtx::shared_from(&read_ctx);
        let spec = sample_spec();
        let entity = Sample { id: 3, a: 11, b: Some("persisted".into()) };

        store_entity(&spec, &write_ctx, &entity, true).unwrap();
        let loaded = compose_entity(&spec, &read_ctx, 3).unwrap().unwrap();
        assert_eq!(loaded.a, 11);
        assert_eq!(loaded.b.as_deref(), Some("persisted"));
    }
}
