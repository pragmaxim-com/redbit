use crate::macro_utils;
use crate::pk::PointerType;
use proc_macro2::Ident;
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{Data, DeriveInput, Field, Fields, GenericArgument, ItemStruct, PathArguments, Type};

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
#[allow(clippy::large_enum_variant)]
pub enum KeyDef {
    Pk(FieldDef),
    Fk { field_def: FieldDef, multiplicity: Multiplicity, parent_type: Option<Type> },
}
impl KeyDef {
    pub fn is_root(&self) -> bool {
        match self {
            KeyDef::Pk(_) => true,
            KeyDef::Fk { .. } => false,
        }
    }
    pub fn field_def(&self) -> FieldDef {
        match self {
            KeyDef::Pk(field_def) => field_def.clone(),
            KeyDef::Fk { field_def, .. } => field_def.clone(),
        }
    }
}

#[derive(Clone)]
pub enum IndexingType {
    Off,
    On { dictionary: bool, range: bool, cache_weight: usize },
}

#[derive(Clone)]
pub struct WriteFrom(pub Ident);
#[derive(Clone)]
pub struct ReadFrom {
    pub outer: Ident,
    pub inner: Ident
}

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ColumnDef {
    Key(KeyDef),
    Plain(FieldDef, IndexingType),
    Relationship(FieldDef, Option<WriteFrom>, Multiplicity),
    Transient(FieldDef, Option<ReadFrom>),
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

fn parse_entity_field(field: &Field) -> syn::Result<ColumnDef> {
    match &field.ident {
        None => Err(syn::Error::new(field.span(), "Unnamed fields not supported")),
        Some(column_name) => {
            let column_type = field.ty.clone();
            for attr in &field.attrs {
                if attr.path().is_ident("pk") {
                    let key_def = KeyDef::Pk(FieldDef { name: column_name.clone(), tpe: column_type.clone() });
                    return Ok(ColumnDef::Key(key_def));
                } else if attr.path().is_ident("fk") {
                    let mut multiplicity = None;
                    let mut parent_type = None;
                    let _ = attr.parse_nested_meta(|nested| {
                        if nested.path.is_ident("one2many") {
                            multiplicity = Some(Multiplicity::OneToMany);
                            parent_type = Some(extract_base_type_from_pointer(field)?);
                        } else if nested.path.is_ident("one2one") {
                            multiplicity = Some(Multiplicity::OneToOne);
                        } else if nested.path.is_ident("one2opt") {
                            multiplicity = Some(Multiplicity::OneToOption);
                        }
                        Ok(())
                    });
                    return if let Some(m) = multiplicity {
                        let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                        Ok(ColumnDef::Key(KeyDef::Fk { field_def: field.clone(), multiplicity: m, parent_type }))
                    } else {
                        Err(syn::Error::new(attr.span(), "Foreign key must specify either `one2many` or `one2one`"))
                    }
                } else if attr.path().is_ident("column") {
                    let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                    let mut indexing = ColumnDef::Plain(field.clone(), IndexingType::Off);
                    let _ = attr.parse_nested_meta(|nested| {
                        let mut cache_weight = 0;
                        let ident = nested.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                        if ident == "cache" {
                            let lit: syn::LitInt = nested.value()?.parse()?;
                            cache_weight = lit.base10_parse::<usize>()?;
                        } else if ["dictionary", "range", "index"].contains(&ident.as_str()) {
                            if nested.input.peek(syn::token::Paren) {
                                nested.parse_nested_meta(|inner| {
                                    if inner.path.is_ident("cache") {
                                        let lit: syn::LitInt = inner.value()?.parse()?;
                                        cache_weight = lit.base10_parse::<usize>()?;
                                    }
                                    Ok(())
                                })?;
                            }
                        } else if ident == "transient" {
                            // valid, nothing to do
                        } else {
                            return Err(syn::Error::new(
                                nested.path.span(),
                                "Cache must be used like this `column(cache = 10)`, `column(dictionary(cache = 10))`, `column(range(cache = 10))`, `column(index(cache = 10))`",
                            ));
                        }
                        if nested.path.is_ident("transient") {
                            let mut read_from: Option<ReadFrom> = None;
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
                            indexing = ColumnDef::Transient(field.clone(), read_from);
                        } else if nested.path.is_ident("index") {
                            indexing = ColumnDef::Plain(field.clone(), IndexingType::On { dictionary: false, range: false, cache_weight });
                        } else if nested.path.is_ident("dictionary") {
                            indexing = ColumnDef::Plain(field.clone(), IndexingType::On { dictionary: true, range: false, cache_weight });
                        } else if nested.path.is_ident("range") {
                            indexing = ColumnDef::Plain(field.clone(), IndexingType::On { dictionary: false, range: true, cache_weight });
                        }
                        Ok(())
                    });
                    return Ok(indexing);
                }
            }
            if let Type::Path(type_path) = &column_type {
                if let Some(segment) = type_path.path.segments.last() {
                    match segment.ident.to_string().as_str() {
                        "Vec" => {
                            // one-to-many
                            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                if let Some(GenericArgument::Type(Type::Path(inner_type_path))) = args.args.first() {
                                    let inner_type = inner_type_path
                                        .path
                                        .segments
                                        .last()
                                        .ok_or_else(|| syn::Error::new(field.span(), "Parent field missing"))?
                                        .ident
                                        .clone();

                                    validate_one_to_many_name(&column_name, &inner_type, field.span())?;

                                    let type_path = Type::Path(syn::TypePath {
                                        qself: None,
                                        path: syn::Path::from(inner_type.clone()),
                                    });
                                    let field_def = FieldDef {
                                        name: column_name.clone(),
                                        tpe: type_path,
                                    };

                                    let write_from: Option<WriteFrom> =
                                        field.attrs.iter()
                                            .find(|attr| attr.path().is_ident("write_from"))
                                            .and_then(|attr| {
                                                let mut field_ref_name: Option<WriteFrom> = None;
                                                let _ = attr.parse_nested_meta(|nested| {
                                                    if let Some(nested_ident) = nested.path.get_ident() {
                                                        field_ref_name = Some(WriteFrom(nested_ident.clone()));
                                                    }
                                                    Ok(())
                                                });
                                                field_ref_name
                                            });

                                    return Ok(ColumnDef::Relationship(field_def, write_from, Multiplicity::OneToMany));
                                }
                            }
                        }
                        "Option" => {
                            // one-to-option
                            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                if let Some(GenericArgument::Type(Type::Path(inner_type_path))) = args.args.first() {
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
                                    return Ok(ColumnDef::Relationship(field, None, Multiplicity::OneToOption));
                                }
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
                                return Ok(ColumnDef::Relationship(field, None, Multiplicity::OneToOne));
                            }
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
    if field_name.to_string() != expected.to_string() {
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

pub fn get_field_macros(ast: &ItemStruct) -> syn::Result<(KeyDef, Vec<ColumnDef>)> {
    let mut key_column: Option<KeyDef> = None;
    let mut columns: Vec<ColumnDef> = Vec::new();

    let fields = get_named_fields(ast)?;

    for field in fields.iter() {
        match parse_entity_field(field)? {
            ColumnDef::Key(key_def) => {
                if key_column.is_some() {
                    return Err(syn::Error::new(field.span(), "Multiple `#[pk]` columns found; only one is allowed"));
                }
                key_column = Some(key_def);
            }
            column => columns.push(column),
        }
    }

    let key = key_column.ok_or_else(|| syn::Error::new(ast.span(), "`#[pk]` or `#[fk] attribute not found on any column."))?;
    columns.insert(0, ColumnDef::Key(key.clone()));
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

