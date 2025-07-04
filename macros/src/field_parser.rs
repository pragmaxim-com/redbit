use proc_macro2::Ident;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{Data, DeriveInput, Field, Fields, GenericArgument, ItemStruct, PathArguments, Type};
use crate::pk::PointerType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Multiplicity {
    OneToOption,
    OneToOne,
    OneToMany,
}

#[derive(Clone)]
pub struct FieldDef {
    pub name: Ident,
    pub tpe: Type,
}

#[derive(Clone)]
pub enum IndexingType {
    Off,
    On { dictionary: bool, range: bool },
}

#[derive(Clone)]
pub enum ColumnDef {
    Key { field_def: FieldDef, fk: Option<Multiplicity> },
    Plain(FieldDef, IndexingType),
    Relationship(FieldDef, Multiplicity),
    Transient(FieldDef),
}

pub fn get_named_fields(ast: &ItemStruct) -> Result<Punctuated<syn::Field, Comma>, syn::Error> {
    match &ast.fields {
        Fields::Named(columns_named) => Ok(columns_named.named.clone()),
        _ => Err(syn::Error::new(ast.span(), "`#[derive(Entity)]` only supports structs with named columns.")),
    }
}

fn parse_entity_field(field: &syn::Field) -> Result<ColumnDef, syn::Error> {
    match &field.ident {
        None => Err(syn::Error::new(field.span(), "Unnamed fields not supported")),
        Some(column_name) => {
            let column_type = field.ty.clone();
            for attr in &field.attrs {
                if attr.path().is_ident("pk") {
                    let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                    return Ok(ColumnDef::Key { field_def: field.clone(), fk: None });
                } else if attr.path().is_ident("fk") {
                    let mut fk = None;
                    let _ = attr.parse_nested_meta(|nested| {
                        if nested.path.is_ident("one2many") {
                            fk = Some(Multiplicity::OneToMany);
                        } else if nested.path.is_ident("one2one") {
                            fk = Some(Multiplicity::OneToOne);
                        } else if nested.path.is_ident("one2opt") {
                            fk = Some(Multiplicity::OneToOption);
                        }
                        Ok(())
                    });
                    if fk.is_none() {
                        return Err(syn::Error::new(attr.span(), "Foreign key must specify either `one2many` or `one2one`"));
                    }
                    let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                    return Ok(ColumnDef::Key { field_def: field.clone(), fk });
                } else if attr.path().is_ident("column") {
                    let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                    let mut indexing = ColumnDef::Plain(field.clone(), IndexingType::Off);
                    let _ = attr.parse_nested_meta(|nested| {
                        if nested.path.is_ident("transient") {
                            indexing = ColumnDef::Transient(field.clone());
                        } else if nested.path.is_ident("index") {
                            indexing = ColumnDef::Plain(field.clone(), IndexingType::On { dictionary: false, range: false });
                        } else if nested.path.is_ident("dictionary") {
                            indexing = ColumnDef::Plain(field.clone(), IndexingType::On { dictionary: true, range: false });
                        } else if nested.path.is_ident("range") {
                            indexing = ColumnDef::Plain(field.clone(), IndexingType::On { dictionary: false, range: true });
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
                                    let type_path = Type::Path(syn::TypePath {
                                        qself: None,
                                        path: syn::Path::from(inner_type),
                                    });
                                    let field = FieldDef {
                                        name: column_name.clone(),
                                        tpe: type_path,
                                    };
                                    return Ok(ColumnDef::Relationship(field, Multiplicity::OneToMany));
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
                                    return Ok(ColumnDef::Relationship(field, Multiplicity::OneToOption));
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
                                return Ok(ColumnDef::Relationship(field, Multiplicity::OneToOne));
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

pub fn get_field_macros(fields: &Punctuated<syn::Field, Comma>, ast: &ItemStruct) -> Result<(FieldDef, Vec<ColumnDef>), syn::Error> {
    let mut pk_column: Option<FieldDef> = None;
    let mut columns: Vec<ColumnDef> = Vec::new();

    for field in fields.iter() {
        match parse_entity_field(field)? {
            ColumnDef::Key { field_def, fk } => {
                if pk_column.is_some() {
                    return Err(syn::Error::new(field.span(), "Multiple `#[pk]` columns found; only one is allowed"));
                }
                pk_column = Some(field_def.clone());
                columns.push(ColumnDef::Key { field_def, fk });
            }
            column => columns.push(column),
        }
    }

    let pk = pk_column.ok_or_else(|| syn::Error::new(ast.span(), "`#[pk]` attribute not found on any column. Exactly one column must have `#[pk]`."))?;

    Ok((pk, columns))
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

            let index_field = match fields.iter().find(|f| is_index_field(&f)) {
                Some(f) => f.clone(),
                None => return Err(syn::Error::new_spanned(input, "Unable to find index field")),
            };
            let parent_field = match fields.iter().find(|f| !is_index_field(&f)) {
                Some(f) => f.clone(),
                None => return Err(syn::Error::new_spanned(input, "Unable to find parent field")),
            };

            Ok((Some(parent_field), index_field))
        }
    }
}

fn is_index_field(f: &Field) -> bool {
    f.ident.as_ref().map_or(false, |name| name.to_string().eq("index"))
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

