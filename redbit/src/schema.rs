use std::any::TypeId;
use thiserror::Error;

/// Relationship cardinality.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Multiplicity {
    OneToOption,
    OneToOne,
    OneToMany,
}

/// Column cache/shard parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnProps {
    pub shards: usize,
    pub db_cache_weight: usize,
    pub lru_cache_size: usize,
}

impl ColumnProps {
    pub fn new(shards: usize, db_cache_weight: usize, lru_cache_size_m: usize) -> Self {
        ColumnProps { shards, db_cache_weight, lru_cache_size: lru_cache_size_m * 1_000_000 }
    }
    pub fn for_key(db_cache_weight: usize) -> Self {
        ColumnProps { shards: 1, db_cache_weight, lru_cache_size: 0 }
    }
}

/// Column indexing mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexingType {
    Off(ColumnProps),
    Index(ColumnProps),
    Range(ColumnProps),
    Dict(ColumnProps),
}

/// Field metadata without macros.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldDef {
    pub name: &'static str,
    pub ty: TypeInfo,
}


/// Minimal type identity used for manual schema.
#[derive(Clone, Copy, Debug)]
pub struct TypeInfo {
    pub id: TypeId,
    pub name: &'static str,
}

impl PartialEq for TypeInfo {
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
}
impl Eq for TypeInfo {}

impl TypeInfo {
    pub fn of<T: 'static>() -> Self {
        TypeInfo { id: TypeId::of::<T>(), name: std::any::type_name::<T>() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyDef {
    Pk { field_def: FieldDef, column_props: ColumnProps },
    Fk { field_def: FieldDef, multiplicity: Multiplicity, parent_type: Option<TypeInfo>, column_props: ColumnProps },
}

impl KeyDef {
    pub fn is_root(&self) -> bool {
        matches!(self, KeyDef::Pk { .. })
    }
    pub fn field_def(&self) -> FieldDef {
        match self {
            KeyDef::Pk { field_def, .. } => field_def.clone(),
            KeyDef::Fk { field_def, .. } => field_def.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Used;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteFrom {
    pub from: &'static str,
    pub using: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadFrom {
    pub outer: &'static str,
    pub inner: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnDef {
    Key(KeyDef),
    Plain(FieldDef, IndexingType, Option<Used>, bool),
    Relationship(FieldDef, Option<WriteFrom>, Option<Used>, Multiplicity),
    Transient(FieldDef),
    TransientRel(FieldDef, Option<ReadFrom>),
}

/// Errors for schema wiring.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SchemaError {
    #[error("dependency not satisfied: expected field `{0}` (Plain or Relationship)")]
    MissingDependency(&'static str),
    #[error("internal invariant: expected Plain or Relationship")]
    InvalidDependency,
    #[error("field `{field}` type mismatch: expected `{expected}`, got `{actual}`")]
    TypeMismatch { field: &'static str, expected: &'static str, actual: &'static str },
    #[error("field `{field}` kind mismatch")]
    KindMismatch { field: &'static str },
}

#[derive(Clone)]
struct Dependency {
    uses: &'static str,
    _used_by: &'static str,
}

fn extract_dependency(col: &ColumnDef) -> Option<Dependency> {
    if let ColumnDef::Relationship(FieldDef { name: used_by, .. }, Some(WriteFrom { using, .. }), ..) = col {
        Some(Dependency { uses: *using, _used_by: *used_by })
    } else {
        None
    }
}

fn find_dependency_by_idx(cols: &[ColumnDef]) -> Option<(usize, Dependency)> {
    cols.iter()
        .enumerate()
        .find_map(|(i, c)| extract_dependency(c).map(|dep| (i, dep)))
}

fn find_used_idx_for_dependency(cols: &[ColumnDef], dep: &Dependency, skip_idx: usize) -> Option<usize> {
    cols.iter().enumerate().position(|(i, c)| {
        if i == skip_idx { return false; }
        match c {
            ColumnDef::Plain(FieldDef { name, .. }, ..) if name == &dep.uses => true,
            ColumnDef::Relationship(FieldDef { name, .. }, ..) if name == &dep.uses => true,
            _ => false,
        }
    })
}

/// Stabilizes dependency ordering (dependee then depender) and sets Used markers.
/// Mirrors the macro-based logic but runs without proc-macros.
pub fn take_and_chain_relation_with_plain(columns: &mut Vec<ColumnDef>) -> Result<Option<(ColumnDef, &'static str)>, SchemaError> {
    let Some((dep_idx, dep)) = find_dependency_by_idx(columns) else {
        return Ok(None);
    };

    let Some(used_idx) = find_used_idx_for_dependency(columns, &dep, dep_idx) else {
        return Err(SchemaError::MissingDependency(dep.uses));
    };

    let (mut used_col, using_col) = if used_idx > dep_idx {
        (columns.remove(used_idx), columns.remove(dep_idx))
    } else {
        let depcol = columns.remove(dep_idx);
        let depd = columns.remove(used_idx);
        (depd, depcol)
    };

    match &mut used_col {
        ColumnDef::Plain(_, _, used_by_slot, _) => {
            *used_by_slot = Some(Used);
        }
        ColumnDef::Relationship(_, _write_from, used_by_rel_slot, _) => {
            *used_by_rel_slot = Some(Used);
        }
        _ => return Err(SchemaError::InvalidDependency),
    }

    columns.push(used_col);
    columns.push(using_col.clone());

    Ok(Some((using_col, dep.uses)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::discriminant;

    fn fd<T: 'static>(name: &'static str) -> FieldDef {
        FieldDef { name, ty: TypeInfo::of::<T>() }
    }

    fn plain<T: 'static>(name: &'static str, used: Option<Used>) -> ColumnDef {
        ColumnDef::Plain(
            fd::<T>(name),
            IndexingType::Off(ColumnProps::for_key(0)),
            used,
            false,
        )
    }

    fn relation<T: 'static>(
        rel_name: &'static str,
        write_from: Option<(&'static str, &'static str)>,
        used: Option<Used>,
        mult: Multiplicity,
    ) -> ColumnDef {
        ColumnDef::Relationship(
            fd::<T>(rel_name),
            write_from.map(|(from, uses)| WriteFrom { from, using: uses }),
            used,
            mult,
        )
    }

    #[test]
    fn ok_none_if_no_dependency_present() {
        let mut cols = vec![
            plain::<u64>("hash", None),
            relation::<u8>("inputs", None, None, Multiplicity::OneToMany),
            plain::<u64>("z", None),
        ];
        let snapshot = cols.clone();
        let out = take_and_chain_relation_with_plain(&mut cols).unwrap();
        assert!(out.is_none());
        assert_eq!(cols.len(), snapshot.len());
    }

    #[test]
    fn err_if_dependency_missing_by_name() {
        let mut cols = vec![
            plain::<u64>("a", None),
            relation::<u8>("inputs", Some(("input_refs", "hash")), None, Multiplicity::OneToMany),
            plain::<u64>("z", None),
        ];
        let snapshot = cols.clone();

        let err = take_and_chain_relation_with_plain(&mut cols).unwrap_err();
        assert_eq!(err, SchemaError::MissingDependency("hash"));
        assert_eq!(cols.len(), snapshot.len());
    }

    #[test]
    fn ok_some_depends_on_plain_sets_used_by_and_moves_tail() {
        let mut cols = vec![
            plain::<u64>("a", None),
            plain::<u64>("hash", None),
            relation::<u8>("inputs", Some(("input_refs", "hash")), None, Multiplicity::OneToMany),
            plain::<u64>("z", None),
        ];

        let out = take_and_chain_relation_with_plain(&mut cols).unwrap();
        assert!(out.is_some());
        let (depender_col, dependee_ident) = out.unwrap();
        assert_eq!(dependee_ident, "hash");

        let n = cols.len();
        match (&cols[n - 2], &cols[n - 1]) {
            (
                ColumnDef::Plain(FieldDef { name: name_plain, .. }, _, Some(_), _),
                ColumnDef::Relationship(FieldDef { name: name_rel, .. }, Some(WriteFrom { using, from }), _ub_rel, mult),
            ) => {
                assert_eq!(name_plain, &"hash");
                assert_eq!(name_rel, &"inputs");
                assert_eq!(using, &"hash");
                assert_eq!(from, &"input_refs");
                assert_eq!(discriminant(mult), discriminant(&Multiplicity::OneToMany));
            }
            _ => panic!("tail elements not as expected for Plain dependee"),
        }

        match (&depender_col, &cols[n - 1]) {
            (
                ColumnDef::Relationship(_, Some(WriteFrom { using: u1, from: f1 }), _ub1, m1),
                ColumnDef::Relationship(_, Some(WriteFrom { using: u2, from: f2 }), _ub2, m2),
            ) => {
                assert_eq!(u1, u2);
                assert_eq!(f1, f2);
                assert_eq!(discriminant(m1), discriminant(m2));
            }
            _ => panic!("returned depender column mismatch"),
        }
    }

    #[test]
    fn ok_some_depends_on_relationship_sets_used_by_and_moves_tail() {
        let mut cols = vec![
            relation::<u8>("hash_rel", None, None, Multiplicity::OneToOne),
            plain::<u64>("a", None),
            relation::<u8>("inputs", Some(("input_refs", "hash_rel")), None, Multiplicity::OneToMany),
            plain::<u64>("z", None),
        ];

        let out = take_and_chain_relation_with_plain(&mut cols).unwrap();
        assert!(out.is_some());
        let (depender_col, dependee_ident) = out.unwrap();
        assert_eq!(dependee_ident, "hash_rel");

        let n = cols.len();
        match (&cols[n - 2], &cols[n - 1]) {
            (
                ColumnDef::Relationship(FieldDef { name: dep_name, .. }, _wf_none, Some(_), mult_dependee),
                ColumnDef::Relationship(FieldDef { name: name_rel, .. }, Some(WriteFrom { using, from }), _ub_rel2, mult_depender),
            ) => {
                assert_eq!(dep_name, &"hash_rel");
                assert_eq!(name_rel, &"inputs");
                assert_eq!(using, &"hash_rel");
                assert_eq!(from, &"input_refs");
                assert_eq!(discriminant(mult_dependee), discriminant(&Multiplicity::OneToOne));
                assert_eq!(discriminant(mult_depender), discriminant(&Multiplicity::OneToMany));
            }
            _ => panic!("tail elements not as expected for Relationship dependee"),
        }

        match (&depender_col, &cols[n - 1]) {
            (
                ColumnDef::Relationship(_, Some(WriteFrom { using: u1, from: f1 }), _ub1, m1),
                ColumnDef::Relationship(_, Some(WriteFrom { using: u2, from: f2 }), _ub2, m2),
            ) => {
                assert_eq!(u1, u2);
                assert_eq!(f1, f2);
                assert_eq!(discriminant(m1), discriminant(m2));
            }
            _ => panic!("returned depender column mismatch"),
        }
    }
}
