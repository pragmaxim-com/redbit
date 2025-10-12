use crate::entity::context::TxType;
use crate::entity::{context, info, query};
use crate::macro_utils;
use crate::pk::PointerType;
use proc_macro2::Ident;
use quote::ToTokens;
use std::fmt::Debug;
use syn::meta::ParseNestedMeta;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{Attribute, Data, DeriveInput, Field, Fields, GenericArgument, ItemStruct, PathArguments, Type};

#[derive(Clone)]
#[allow(clippy::enum_variant_names)]
pub enum Multiplicity {
    OneToOption,
    OneToOne,
    OneToMany,
}

#[derive(Clone)]
pub struct OneToManyParentDef {
    pub parent_type: Type,
    pub parent_ident: Ident,
    pub stream_query_ty: Type,
    pub tx_context_ty: Type
}

#[derive(Clone)]
pub struct FieldDef {
    pub name: Ident,
    pub tpe: Type,
}

#[derive(Clone)]
pub struct EntityDef {
    pub key_def: KeyDef,
    pub entity_name: Ident,
    pub entity_type: Type,
    pub query_type: Type,
    pub info_type: Type,
    pub read_ctx_type: Type,
    pub write_ctx_type: Type,
}

impl EntityDef {
    pub fn _fq_pk_name(&self) -> String {
        format!("{}_{}", self.entity_name.to_string().to_lowercase(), self.key_def.field_def().name)
    }
}

impl EntityDef {
    pub fn new(key_def: KeyDef, entity_name: Ident, entity_type: Type) -> Self {
        let query_type = query::filter_query_type(&entity_type);
        let read_ctx_type = context::entity_tx_context_type(&entity_type, TxType::Read);
        let write_ctx_type = context::entity_tx_context_type(&entity_type, TxType::Write);
        let info_type = info::table_info_type(&entity_type);
        EntityDef { key_def, entity_name, entity_type, query_type, info_type, read_ctx_type, write_ctx_type }
    }
}

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum KeyDef {
    Pk { field_def: FieldDef, column_props: ColumnProps },
    Fk { field_def: FieldDef, multiplicity: Multiplicity, parent_type: Option<Type>, column_props: ColumnProps },
}
impl KeyDef {
    pub fn is_root(&self) -> bool {
        match self {
            KeyDef::Pk { .. } => true,
            KeyDef::Fk { .. } => false,
        }
    }
    pub fn field_def(&self) -> FieldDef {
        match self {
            KeyDef::Pk { field_def, .. } => field_def.clone(),
            KeyDef::Fk { field_def, .. } => field_def.clone(),
        }
    }
}

#[derive(Clone)]
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
        ColumnProps { shards: 0, db_cache_weight, lru_cache_size: 0 }
    }
}

#[derive(Clone)]
pub enum IndexingType {
    Off(ColumnProps),
    Index(ColumnProps),
    Range(ColumnProps),
    Dict(ColumnProps),
}

#[derive(Clone, Debug)]
pub struct WriteFrom {
    pub from: Ident,
    pub using: Ident,
}
#[derive(Clone)]
pub struct ReadFrom {
    pub outer: Ident,
    pub inner: Ident
}

#[derive(Clone, Debug)]
pub struct Used;

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ColumnDef {
    Key(KeyDef),
    Plain(FieldDef, IndexingType, Option<Used>),
    Relationship(FieldDef, Option<WriteFrom>, Option<Used>, Multiplicity),
    Transient(FieldDef, Option<ReadFrom>),
}

impl Debug for ColumnDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColumnDef::Key(k) => write!(f, "Key({})", k.field_def().name),
            ColumnDef::Plain(field, indexing_type, used_by) => {
                let index_str = match indexing_type {
                    IndexingType::Off(_) => "Off",
                    IndexingType::Index(_) => "Index",
                    IndexingType::Range(_) => "Range",
                    IndexingType::Dict(_) => "Dict",
                };
                if let Some(Used) = used_by {
                    write!(f, "Plain({}, {}, UsedBy)", field.name, index_str)
                } else {
                    write!(f, "Plain({}, {}, No UsedBy)", field.name, index_str)
                }
            }
            ColumnDef::Relationship(field, write_from_using, used_by, multiplicity) => {
                let mult_str = match multiplicity {
                    Multiplicity::OneToMany => "OneToMany",
                    Multiplicity::OneToOne => "OneToOne",
                    Multiplicity::OneToOption => "OneToOption",
                };
                if let Some(Used) = used_by {
                    if let Some(WriteFrom { from, using }) = write_from_using {
                        write!(f, "Relationship({}, {}, WriteFrom(from: {}, using: {}), UsedBy)", field.name, mult_str, from, using)
                    } else {
                        write!(f, "Relationship({}, {}, No WriteFrom, UsedBy)", field.name, mult_str)
                    }
                } else {
                    if let Some(WriteFrom { from, using }) = write_from_using {
                        write!(f, "Relationship({}, {}, WriteFrom(from: {}, using: {}), No UsedBy)", field.name, mult_str, from, using)
                    } else {
                        write!(f, "Relationship({}, {}, No WriteFrom, No UsedBy)", field.name, mult_str)
                    }
                }
            }
            ColumnDef::Transient(field, read_from) => {
                if let Some(ReadFrom { outer, inner }) = read_from {
                    write!(f, "Transient({}, ReadFrom(outer: {}, inner: {}))", field.name, outer, inner)
                } else {
                    write!(f, "Transient({}, No ReadFrom)", field.name)
                }
            }
        }
    }
}

pub fn get_named_fields(ast: &ItemStruct) -> syn::Result<Punctuated<Field, Comma>> {
    match &ast.fields {
        Fields::Named(columns_named) => Ok(columns_named.named.clone()),
        _ => Err(syn::Error::new(ast.span(), "`#[derive(Entity)]` only supports structs with named columns.")),
    }
}

pub fn extract_base_type_from_pointer(field: &Field) -> syn::Result<Type> {
    match &field.ty {
        Type::Path(type_path) => {
            if let Some(seg) = type_path.path.segments.last() {
                let ident_str = seg.ident.to_string();

                if let Some(base_name) = ident_str.strip_suffix("Pointer") {
                    syn::parse_str::<Type>(base_name).map_err(|e| {
                        syn::Error::new_spanned(&seg.ident, format!("Failed to parse key type `{}`: {}", base_name, e))
                    })
                } else {
                    Err(syn::Error::new_spanned(
                        &seg.ident,
                        format!("Expected Key type of format `ParentPointer`, found `{}`", ident_str),
                    ))
                }
            } else {
                Err(syn::Error::new_spanned(
                    &type_path.path,
                    format!("Expected Key type of format `ParentPointer`, found `{:?}`", type_path.to_token_stream())
                ))
            }
        }
        other => Err(syn::Error::new_spanned(
            other,
            format!("Expected Key type of format `ParentPointer`, found `{:?}`", other.to_token_stream()),
        )),
    }
}

#[inline]
fn parse_write_from_using(attr: &Attribute) -> syn::Result<WriteFrom> {
    // Collect comma-separated metas: input_refs, hash
    let mut idents: Vec<Ident> = Vec::with_capacity(2);

    attr.parse_nested_meta(|nested| {
        if let Some(id) = nested.path.get_ident() {
            idents.push(id.clone());
            Ok(())
        } else {
            // Anything non-ident inside the list -> uniform error text
            Err(syn::Error::new_spanned(
                &nested.path,
                "expected #[write_from_using(from_ident, using_ident)]",
            ))
        }
    })?;

    match idents.as_slice() {
        [from, using] => Ok(WriteFrom { from: from.clone(), using: using.clone() }),
        _ => Err(syn::Error::new_spanned(
            attr,
            "expected #[write_from_using(from_ident, using_ident)]",
        )),
    }
}

fn get_column_usize_attr(nested: &ParseNestedMeta, attr: &str) -> Result<Option<usize>, syn::Error> {
    let ident = nested.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
    match ident.as_str() {
        i if attr == i => {
            let lit: syn::LitInt = nested.value()?.parse()?;
            Ok(Some(lit.base10_parse::<usize>()?))
        }
        "dictionary" | "range" | "index" | "transient" => {
            Ok(None)
        }
        _ => {
            Err(syn::Error::new(
                nested.path.span(),
                "Unsupported form. Use `db_cache = 10`, `dictionary`, `range`, `index`, or `transient`",
            ))
        }
    }
}

fn parse_entity_field(field: &Field) -> syn::Result<ColumnDef> {
    match &field.ident {
        None => Err(syn::Error::new(field.span(), "Unnamed fields not supported")),
        Some(column_name) => {
            let column_type = field.ty.clone();
            for attr in &field.attrs {
                if attr.path().is_ident("pk") {
                    let mut db_cache_weight = 0;
                    let _ = attr.parse_nested_meta(|nested| {
                        db_cache_weight = get_column_usize_attr(&nested, "db_cache")?.unwrap_or(0);
                        Ok(())
                    });
                    let column_props = ColumnProps::for_key(db_cache_weight);
                    let key_def = KeyDef::Pk { field_def: FieldDef { name: column_name.clone(), tpe: column_type.clone() }, column_props };
                    return Ok(ColumnDef::Key(key_def));
                } else if attr.path().is_ident("fk") {
                    let mut multiplicity = None;
                    let mut parent_type = None;
                    let mut db_cache_weight = 0;
                    let _ = attr.parse_nested_meta(|nested| {
                        let ident = nested.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                        match ident.as_str() {
                            "db_cache" => {
                                let lit: syn::LitInt = nested.value()?.parse()?;
                                db_cache_weight = lit.base10_parse::<usize>()?;
                            }
                            "one2many" => {
                                multiplicity = Some(Multiplicity::OneToMany);
                                parent_type = Some(extract_base_type_from_pointer(field)?);
                            }
                            "one2one" => {
                                multiplicity = Some(Multiplicity::OneToOne);
                            }
                            "one2opt" => {
                                multiplicity = Some(Multiplicity::OneToOption);
                            }
                            _ => {
                                return Err(syn::Error::new(
                                    nested.path.span(),
                                    "Unsupported form. Use `fk(one2many/one2one/one2opt, db_cache = 10)` or `fk(one2many/one2one/one2opt)`",
                                ));
                            }
                        }
                        Ok(())
                    });
                    return if let Some(m) = multiplicity {
                        let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                        let column_props = ColumnProps::for_key(db_cache_weight);
                        Ok(ColumnDef::Key(KeyDef::Fk { field_def: field.clone(), multiplicity: m, parent_type, column_props }))
                    } else {
                        Err(syn::Error::new(attr.span(), "Foreign key must specify either `one2many` or `one2one`"))
                    }
                } else if attr.path().is_ident("column") {
                    let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                    let mut used = false;
                    let mut db_cache_weight = 0;
                    let mut lru_cache_size_mil = 0;
                    let mut shards = 0;
                    let mut is_index = false;
                    let mut is_dictionary = false;
                    let mut is_range = false;
                    let mut is_transient = false;
                    let mut read_from: Option<ReadFrom> = None;

                    let _ = attr.parse_nested_meta(|nested| {
                        if nested.path.is_ident("db_cache") {
                            let lit: syn::LitInt = nested.value()?.parse()?;
                            db_cache_weight = lit.base10_parse::<usize>()?;
                        } else if nested.path.is_ident("used") {
                            used = true;
                        } else if nested.path.is_ident("lru_cache") {
                            let lit: syn::LitInt = nested.value()?.parse()?;
                            lru_cache_size_mil = lit.base10_parse::<usize>()?;
                        } else if nested.path.is_ident("shards") {
                            let lit: syn::LitInt = nested.value()?.parse()?;
                            shards = lit.base10_parse::<usize>()?;
                        } else if nested.path.is_ident("transient") {
                            is_transient = true;
                            let _ = nested.parse_nested_meta(|inner| {
                                if inner.path.is_ident("read_from") {
                                    let _ = inner.parse_nested_meta(|leaf| {
                                        let p = leaf.path.clone();
                                        if let [outer_ident, inner_ident] = p.segments.iter().map(|s| s.ident.clone()).collect::<Vec<Ident>>().as_slice() {
                                            read_from = Some(ReadFrom { outer: outer_ident.clone(), inner: inner_ident.clone() });
                                        } else {
                                            return Err(syn::Error::new(attr.span(), "read_from must be a path of format 'one_to_many_entity_field::pointer_ref'"))
                                        }
                                        Ok(())
                                    });
                                }
                                Ok(())
                            });
                        } else if nested.path.is_ident("index") {
                            is_index = true;
                        } else if nested.path.is_ident("dictionary") {
                            is_dictionary = true;
                        } else if nested.path.is_ident("range") {
                            is_range = true;
                        }
                        Ok(())
                    });
                    let column_props = ColumnProps::new(shards, db_cache_weight, lru_cache_size_mil);
                    let column_def = if is_transient {
                        ColumnDef::Transient(field.clone(), read_from)
                    } else if is_dictionary {
                        ColumnDef::Plain(field.clone(), IndexingType::Dict(column_props), None)
                    } else if is_range {
                        ColumnDef::Plain(field.clone(), IndexingType::Range(column_props), None)
                    } else if is_index {
                        ColumnDef::Plain(field.clone(), IndexingType::Index(column_props), None)
                    } else {
                        ColumnDef::Plain(field.clone(), IndexingType::Off(column_props), None)
                    };
                    return Ok(column_def);
                }
            }
            if let Type::Path(type_path) = &column_type && let Some(segment) = type_path.path.segments.last() {
                match segment.ident.to_string().as_str() {
                    "Vec" => {
                        // one-to-many
                        if let PathArguments::AngleBracketed(args) = &segment.arguments
                            && let Some(GenericArgument::Type(Type::Path(inner_type_path))) = args.args.first() {
                                let inner_type = inner_type_path
                                    .path
                                    .segments
                                    .last()
                                    .ok_or_else(|| syn::Error::new(field.span(), "Parent field missing"))?
                                    .ident
                                    .clone();

                                validate_one_to_many_name(column_name, &inner_type, field.span())?;

                                let type_path = Type::Path(syn::TypePath {
                                    qself: None,
                                    path: syn::Path::from(inner_type.clone()),
                                });
                                let field_def = FieldDef {
                                    name: column_name.clone(),
                                    tpe: type_path,
                                };

                            let write_from_using: Option<WriteFrom> =
                                field.attrs.iter()
                                    .find(|attr| attr.path().is_ident("write_from_using"))
                                    .and_then(|attr| parse_write_from_using(attr).ok());

                            return Ok(ColumnDef::Relationship(field_def, write_from_using, None, Multiplicity::OneToMany));
                            }

                    }
                    "Option" => {
                        // one-to-option
                        if let PathArguments::AngleBracketed(args) = &segment.arguments
                            && let Some(GenericArgument::Type(Type::Path(inner_type_path))) = args.args.first() {
                                let inner_type = inner_type_path
                                    .path
                                    .segments
                                    .last()
                                    .ok_or_else(|| syn::Error::new(field.span(), "Parent field missing"))?
                                    .ident
                                    .clone();
                                let type_path = Type::Path(syn::TypePath {
                                    qself: None,
                                    path: syn::Path::from(inner_type),
                                });
                                let field = FieldDef {
                                    name: column_name.clone(),
                                    tpe: type_path,
                                };
                                return Ok(ColumnDef::Relationship(field, None, None, Multiplicity::OneToOption));
                            }

                    }
                    _ => {
                        // one-to-one (plain type)
                        let struct_type = &segment.ident;
                        if segment.arguments.is_empty() {
                            let type_path = Type::Path(syn::TypePath {
                                qself: None,
                                path: syn::Path::from(struct_type.clone()),
                            });
                            let field = FieldDef {
                                name: column_name.clone(),
                                tpe: type_path,
                            };
                            return Ok(ColumnDef::Relationship(field, None, None, Multiplicity::OneToOne));
                        }
                    }
                }
            }
            Err(syn::Error::new(
                field.span(),
                "Field must have one of #[pk(...)] / #[fk(...)] / #[column(...)] / #[transient] annotations or it is a one2one, one2opt, or one2many relationship (e.g., `Vec<Transaction>` or `Option<Transaction>`).",
            ))
        }
    }
}

fn validate_one_to_many_name(
    field_name: &Ident,
    inner_type: &Ident,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    let expected = macro_utils::one_to_many_field_name_from_ident(inner_type);
    if *field_name != expected {
        Err(syn::Error::new(
            span,
            format!(
                "One2many field must be named like a snake_case plural of the underlying entity name: '{}: Vec<{}>' ",
                expected,
                inner_type
            ),
        ))
    } else {
        Ok(())
    }
}


#[derive(Clone)]
struct Dependency {
    /// The field being depended on (dependee), e.g. `hash`
    uses: Ident,
    /// The field that depends on it (depender), e.g. `inputs`
    _used_by: Ident,
}

/// Extracts a `Dependency` from a column that encodes it via `WriteFrom`.
/// Relationship(field(name=used_by), Some(WriteFrom{ using: uses, .. }), _)
#[inline]
fn extract_dependency(col: &ColumnDef) -> Option<Dependency> {
    if let ColumnDef::Relationship(
        FieldDef { name: used_by, .. },
        Some(WriteFrom { using, .. }),
        ..,
    ) = col
    {
        Some(Dependency { uses: using.clone(), _used_by: used_by.clone() })
    } else {
        None
    }
}

/// Find the first depender column and return its index + the Dependency link.
#[inline]
fn find_dependency_by_idx(cols: &[ColumnDef]) -> Option<(usize, Dependency)> {
    cols.iter()
        .enumerate()
        .find_map(|(i, c)| extract_dependency(c).map(|dep| (i, dep)))
}
/// Find the **dependee** index by name, accepting either Plain **or** Relationship.
///
/// - Matches by `FieldDef.name == dep.uses`.
/// - Skips `skip_idx` to avoid self-dependency.
///
#[inline]
fn find_used_idx_for_dependency(cols: &[ColumnDef], dep: &Dependency, skip_idx: usize) -> Option<usize> {
    cols.iter().enumerate().position(|(i, c)| {
        if i == skip_idx { return false; }
        match c {
            ColumnDef::Plain(FieldDef { name, .. }, _, _)
            if name == &dep.uses => true,
            ColumnDef::Relationship(FieldDef { name, .. }, _, _, _)
            if name == &dep.uses => true,
            _ => false,
        }
    })
}

/// Stable removals (shift-left), then appends: **dependee first**, then the depender.
/// Returns:
/// - Ok(None)    : no dependency column present
/// - Err(..)     : dependency present but matching dependee not found by name
/// - Ok(Some(..)): success; columns mutated accordingly
pub fn take_and_chain_relation_with_plain(columns: &mut Vec<ColumnDef>) -> syn::Result<Option<(ColumnDef, Ident)>> {
    let Some((dep_idx, dep)) = find_dependency_by_idx(columns) else {
        return Ok(None);
    };

    let Some(used_idx) = find_used_idx_for_dependency(columns, &dep, dep_idx) else {
        return Err(syn::Error::new(
            dep.uses.span(),
            format!("dependency not satisfied: expected field `{}` (Plain or Relationship)", dep.uses),
        ));
    };

    // Remove in descending index order to keep indices valid.
    let (mut used_col, using_col) = if used_idx > dep_idx {
        (columns.remove(used_idx), columns.remove(dep_idx))
    } else {
        let depcol = columns.remove(dep_idx);
        let depd = columns.remove(used_idx);
        (depd, depcol)
    };

    // Set UsedBy(dep.used_by) on the **dependee**, whether Plain or Relationship.
    match &mut used_col {
        ColumnDef::Plain(_, _, used_by_slot) => {
            *used_by_slot = Some(Used);
        }
        ColumnDef::Relationship(_, _write_from, used_by_rel_slot, _) => {
            *used_by_rel_slot = Some(Used);
        }
        _ => {
            // Defensive: with our selection this should be unreachable.
            return Err(syn::Error::new(dep.uses.span(), "internal invariant: expected Plain or Relationship"));
        }
    }

    // Append in order: dependee first, then depender.
    columns.push(used_col);
    columns.push(using_col.clone());

    Ok(Some((using_col, dep.uses)))
}

pub fn get_field_macros(ast: &ItemStruct) -> syn::Result<(KeyDef, Vec<ColumnDef>)> {
    let mut key_column: Option<KeyDef> = None;
    let mut columns: Vec<ColumnDef> = Vec::new();
    let mut transient_columns: Vec<ColumnDef> = Vec::new();

    let fields = get_named_fields(ast)?;

    for field in fields.iter() {
        match parse_entity_field(field)? {
            ColumnDef::Key(key_def) => {
                if key_column.is_some() {
                    return Err(syn::Error::new(field.span(), "Multiple `#[pk]` columns found; only one is allowed"));
                }
                key_column = Some(key_def);
            }
            ColumnDef::Transient(field_def, read_from) => {
                transient_columns.push(ColumnDef::Transient(field_def, read_from));
            }
            column => columns.push(column),
        }
    }

    let key = key_column.ok_or_else(|| syn::Error::new(ast.span(), "`#[pk]` or `#[fk] attribute not found on any column."))?;
    if !key.is_root() {
        columns.insert(0, ColumnDef::Key(key.clone()));
    }

    take_and_chain_relation_with_plain(&mut columns)?;
    columns.extend(transient_columns);
    if key.is_root() {
        columns.push(ColumnDef::Key(key.clone()));
    }
    Ok((key, columns))
}

/// Extracts and validates the required fields (parent & index) for root or pointer.
pub fn extract_pointer_key_fields(input: &DeriveInput, pointer_type: &PointerType) -> Result<(Option<Field>, Field), syn::Error> {
    let data_struct = match input.data.clone() {
        Data::Struct(data_struct) => data_struct,
        _ => return Err(syn::Error::new_spanned(input, "Pk can only be derived for structs")),
    };

    match pointer_type {
        PointerType::Root => {
            let fields: Vec<_> = match data_struct.fields {
                Fields::Named(fields) => fields.named.into_iter().collect(),
                Fields::Unnamed(fields) => fields.unnamed.into_iter().collect(),
                _ => return Err(syn::Error::new_spanned(input, "Pk must have exactly one field")),
            };

            if fields.len() != 1 {
                return Err(syn::Error::new_spanned(input, "Pk must have exactly one field"));
            }
            Ok((None, fields[0].clone())) // Root has only an index field
        }
        PointerType::Child => {
            let fields: Vec<_> = match data_struct.fields {
                Fields::Named(fields) => fields.named.into_iter().collect(),
                _ => return Err(syn::Error::new_spanned(input, "Pk can only be used with named fields")),
            };
            if fields.len() != 2 {
                return Err(syn::Error::new_spanned(input, "Child struct must have exactly two fields (parent and index)"));
            }

            let index_field = match fields.iter().find(|f| is_index_field(f)) {
                Some(f) => f.clone(),
                None => return Err(syn::Error::new_spanned(input, "Unable to find index field")),
            };
            let parent_field = match fields.iter().find(|f| !is_index_field(f)) {
                Some(f) => f.clone(),
                None => return Err(syn::Error::new_spanned(input, "Unable to find parent field")),
            };

            Ok((Some(parent_field), index_field))
        }
    }
}

fn is_index_field(f: &Field) -> bool {
    f.ident.as_ref().is_some_and(|name| name.to_string().eq("index"))
}

/// Determines whether a struct is a `Root` or `Child` based on `#[parent]` attributes.
pub fn validate_root_key(input: &DeriveInput) -> Result<(), syn::Error> {
    match &input.data {
        Data::Struct(_) => Ok(()),
        _ => Err(syn::Error::new_spanned(input, "Pk can only be derived for structs")),
    }
}

pub fn validate_pointer_key(input: &DeriveInput) -> Result<(), syn::Error> {
    let data_struct = match &input.data {
        Data::Struct(data_struct) => data_struct,
        _ => return Err(syn::Error::new_spanned(input, "Fk can only be derived for structs")),
    };

    let fields: Vec<_> = match &data_struct.fields {
        Fields::Named(fields) => fields.named.iter().collect(),
        _ => return Err(syn::Error::new_spanned(input, "Fk can only be used with named fields")),
    };
    if fields.len() != 2 {
        return Err(syn::Error::new_spanned(input, "Fk must have exactly two fields (parent and index)"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::discriminant;
    use syn::{parse_str, Attribute, Type};

    // -------------------------
    // helpers
    // -------------------------
    fn ident(s: &str) -> syn::Ident { parse_str::<syn::Ident>(s).unwrap() }
    fn mock_type() -> Type { parse_str::<Type>("u64").unwrap() }
    fn fd(name: &str) -> FieldDef { FieldDef { name: ident(name), tpe: mock_type() } }

    fn plain(name: &str, used: Option<Used>) -> ColumnDef {
        ColumnDef::Plain(
            fd(name),
            IndexingType::Off(ColumnProps::for_key(0)),
            used,
        )
    }

    /// Relationship constructor:
    /// - field name = `rel_name`
    /// - write_from = Some(WriteFrom { from, using }) for a **depender**
    ///               or None for a **dependee** relation.
    /// - used_by_rel = Option<UsedBy> on the relation itself (usually None initially)
    fn relation(
        rel_name: &str,
        write_from: Option<(&str, &str)>,
        used: Option<Used>,
        mult: Multiplicity,
    ) -> ColumnDef {
        ColumnDef::Relationship(
            fd(rel_name),
            write_from.map(|(from, uses)| WriteFrom { from: ident(from), using: ident(uses) }),
            used,
            mult,
        )
    }

    // -------------------------
    // parser tests
    // -------------------------
    #[test]
    fn parses_two_idents_positional() {
        let attr: Attribute = syn::parse_quote!(#[write_from_using(input_refs, hash)]);
        let w = super::parse_write_from_using(&attr).unwrap();
        assert_eq!(w.from, ident("input_refs"));
        assert_eq!(w.using, ident("hash"));
    }

    #[test]
    fn errors_on_wrong_arity() {
        let attr0: Attribute = syn::parse_quote!(#[write_from_using()]);
        let attr1: Attribute = syn::parse_quote!(#[write_from_using(only_one)]);
        let attr3: Attribute = syn::parse_quote!(#[write_from_using(a, b, c)]);
        for attr in [attr0, attr1, attr3] {
            let err = super::parse_write_from_using(&attr).unwrap_err();
            assert!(err.to_string().contains("expected #[write_from_using(from_ident, using_ident)]"));
        }
    }

    // -------------------------
    // dependency chain tests
    // -------------------------

    #[test]
    fn ok_none_if_no_dependency_column_present() {
        // No Relationship with WriteFrom present → Ok(None)
        let mut cols = vec![
            plain("hash", None),
            relation("inputs", None, None, Multiplicity::OneToMany), // no WriteFrom => not a depender
            plain("z", None),
        ];
        let snapshot = cols.clone();
        let out = super::take_and_chain_relation_with_plain(&mut cols).unwrap();
        assert!(out.is_none());
        assert_eq!(cols.len(), snapshot.len());
    }

    #[test]
    fn err_if_dependency_present_but_dependee_missing_by_name() {
        // Depender exists (inputs uses hash), but no column named `hash` exists.
        let mut cols = vec![
            plain("a", None),
            relation("inputs", Some(("input_refs", "hash")), None, Multiplicity::OneToMany),
            plain("z", None),
        ];
        let snapshot = cols.clone();

        let err = super::take_and_chain_relation_with_plain(&mut cols).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("dependency not satisfied"));
        assert!(msg.contains("hash"));

        // Atomic failure
        assert_eq!(cols.len(), snapshot.len());
        match (&cols[1], &snapshot[1]) {
            (
                ColumnDef::Relationship(_, Some(WriteFrom { using: u1, from: f1 }), _ub1, m1),
                ColumnDef::Relationship(_, Some(WriteFrom { using: u2, from: f2 }), _ub2, m2),
            ) => {
                assert_eq!(u1, u2);
                assert_eq!(f1, f2);
                assert_eq!(discriminant(m1), discriminant(m2));
            }
            _ => panic!("structure changed unexpectedly"),
        }
    }

    #[test]
    fn ok_some_depends_on_plain_sets_used_by_and_moves_tail() {
        // layout:
        // 0: Plain(a, None)
        // 1: Plain(hash, None)                   <-- dependee (Plain)
        // 2: Relationship(inputs uses hash, ..)  <-- depender
        // 3: Plain(z, None)
        let mut cols = vec![
            plain("a", None),
            plain("hash", None),
            relation("inputs", Some(("input_refs", "hash")), None, Multiplicity::OneToMany),
            plain("z", None),
        ];

        let out = super::take_and_chain_relation_with_plain(&mut cols).unwrap();
        assert!(out.is_some());
        let (depender_col, dependee_ident) = out.unwrap();
        assert_eq!(dependee_ident, ident("hash"));

        // Tail: (Plain(hash, Some(UsedBy(inputs))), Relationship(inputs uses hash, ..))
        let n = cols.len();
        match (&cols[n - 2], &cols[n - 1]) {
            (
                ColumnDef::Plain(FieldDef { name: name_plain, .. }, _, Some(Used)),
                ColumnDef::Relationship(FieldDef { name: name_rel, .. }, Some(WriteFrom { using, from }), _ub_rel, mult),
            ) => {
                assert_eq!(name_plain, &ident("hash"));
                assert_eq!(name_rel, &ident("inputs"));
                assert_eq!(using, &ident("hash"));
                assert_eq!(from, &ident("input_refs"));
                assert_eq!(discriminant(mult), discriminant(&Multiplicity::OneToMany));
            }
            _ => panic!("tail elements not as expected for Plain dependee"),
        }

        // returned depender_col equals the final element
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
        // Now dependee is a **Relationship** (named "hash_rel")
        // 0: Relationship(hash_rel, None, None, ..)     <-- dependee relation (no WriteFrom)
        // 1: Plain(a, None)
        // 2: Relationship(inputs uses hash_rel, ..)     <-- depender relation (has WriteFrom)
        // 3: Plain(z, None)
        let mut cols = vec![
            relation("hash_rel", None, None, Multiplicity::OneToOne),
            plain("a", None),
            relation("inputs", Some(("input_refs", "hash_rel")), None, Multiplicity::OneToMany),
            plain("z", None),
        ];

        let out = super::take_and_chain_relation_with_plain(&mut cols).unwrap();
        assert!(out.is_some());
        let (depender_col, dependee_ident) = out.unwrap();
        assert_eq!(dependee_ident, ident("hash_rel"));

        // Tail: (Relationship(hash_rel, .., Some(UsedBy(inputs)), ..), Relationship(inputs uses hash_rel, ..))
        let n = cols.len();
        match (&cols[n - 2], &cols[n - 1]) {
            (
                ColumnDef::Relationship(
                    FieldDef { name: dep_name, .. },
                    _wf_none,
                    Some(Used),
                    mult_dependee,
                ),
                ColumnDef::Relationship(
                    FieldDef { name: name_rel, .. },
                    Some(WriteFrom { using, from }),
                    _ub_rel2,
                    mult_depender,
                ),
            ) => {
                assert_eq!(dep_name, &ident("hash_rel"));
                assert_eq!(name_rel, &ident("inputs"));
                assert_eq!(using, &ident("hash_rel"));
                assert_eq!(from, &ident("input_refs"));
                assert_eq!(discriminant(mult_dependee), discriminant(&Multiplicity::OneToOne));
                assert_eq!(discriminant(mult_depender), discriminant(&Multiplicity::OneToMany));
            }
            _ => panic!("tail elements not as expected for Relationship dependee"),
        }

        // Head order preserved for remaining elements: a, z
        match (&cols[0], &cols[1]) {
            (ColumnDef::Plain(FieldDef { name: n0, .. }, _, _),
                ColumnDef::Plain(FieldDef { name: n1, .. }, _, _)) => {
                assert_eq!(n0, &ident("a"));
                assert_eq!(n1, &ident("z"));
            }
            _ => panic!("head order not preserved"),
        }

        // returned depender_col equals the final element
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
