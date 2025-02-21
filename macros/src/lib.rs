mod column_macros;
mod entity_macros;
mod pk_macros;
mod relationship_macros;

extern crate proc_macro;

use proc_macro2::Ident;
use syn::{parse_macro_input, punctuated::Punctuated, spanned::Spanned, token::Comma, Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};
use crate::entity_macros::EntityMacros;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Indexing {
    Off,
    On { dictionary: bool, range: bool },
}

enum Multiplicity {
    OneToOne,
    OneToMany
}

enum ParsingResult {
    Pk(Pk),
    Column(Column),
    RelationShip(Relationship),
}

struct Field {
    name: Ident,
    tpe: Type,
}

struct Pk {
    field: Field,
    range: bool,
}

struct Column {
    field: Field,
    indexing: Indexing,
}

struct Relationship {
    field: Field,
    multiplicity: Multiplicity
}

fn get_named_fields(ast: &DeriveInput) -> Result<Punctuated<syn::Field, Comma>, syn::Error> {
    match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(columns_named) => Ok(columns_named.named.clone()),
            _ => Err(syn::Error::new(ast.span(), "`#[derive(Entity)]` only supports structs with named columns.")),
        },
        _ => Err(syn::Error::new(ast.span(), "`#[derive(Entity)]` can only be applied to structs.")),
    }
}

fn parse_entity_field(field: &syn::Field) -> Result<ParsingResult, syn::Error> {
    match &field.ident {
        None => Err(syn::Error::new(field.span(), "Unnamed fields not supported")),
        Some(column_name) => {
            let column_type = field.ty.clone();
            let mut column: Option<ParsingResult> = None;
            for attr in &field.attrs {
                if attr.path().is_ident("pk") {
                    let mut range = false;
                    let _ = attr.parse_nested_meta(|nested| {
                        if nested.path.is_ident("range") {
                            range = true;
                        }
                        Ok(())
                    });
                    if column.is_some() {
                        return Err(syn::Error::new(field.span(), "Only one #[pk] or #[column(...)] annotation allowed per field"));
                    }
                    let field = Field { name: column_name.clone(), tpe: column_type.clone() };
                    column = Some(ParsingResult::Pk(Pk { field, range }));
                }
                if attr.path().is_ident("column") {
                    let mut indexing = Indexing::Off;
                    let _ = attr.parse_nested_meta(|nested| {
                        if nested.path.is_ident("index") {
                            indexing = Indexing::On { dictionary: false, range: false };
                        }
                        if nested.path.is_ident("dictionary") {
                            indexing = Indexing::On { dictionary: true, range: false };
                        } else if nested.path.is_ident("range") {
                            indexing = Indexing::On { dictionary: false, range: true };
                        }
                        Ok(())
                    });
                    if column.is_some() {
                        return Err(syn::Error::new(field.span(), "Only one #[pk] or #[column(...)] annotation allowed per field"));
                    }
                    let field = Field { name: column_name.clone(), tpe: column_type.clone() };
                    column = Some(ParsingResult::Column(Column {
                        field,
                        indexing,
                    }));
                }
            }
            if column.is_none() {
                if let Type::Path(type_path) = &column_type {
                    if let Some(segment) = type_path.path.segments.last() {
                        if segment.ident == "Vec" {
                            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                if let Some(GenericArgument::Type(Type::Path(inner_type_path))) = args.args.first() {
                                    let inner_type = &inner_type_path.path.segments.last().unwrap().ident;
                                    let type_path = syn::Type::Path(syn::TypePath {
                                        qself: None,
                                        path: syn::Path::from(inner_type.clone()),
                                    });
                                    let field = Field {
                                        name: column_name.clone(),
                                        tpe: type_path,
                                    };
                                    column = Some(ParsingResult::RelationShip(Relationship {
                                        field,
                                        multiplicity: Multiplicity::OneToMany,
                                    }));
                                }
                            }
                        } else {
                            let struct_type = &segment.ident;
                            let type_path = syn::Type::Path(syn::TypePath {
                                qself: None,
                                path: syn::Path::from(struct_type.clone()),
                            });
                            let field = Field {
                                name: column_name.clone(),
                                tpe: type_path,
                            };
                            column = Some(ParsingResult::RelationShip(Relationship {
                                field,
                                multiplicity: Multiplicity::OneToOne,
                            }));
                        }
                    }
                }
            }


            column.ok_or_else(|| syn::Error::new(
                field.span(),
                "Field must have either #[pk] / #[column(...)] annotation or be a OneToOne / OneToMany relationship",
            ))
        }
    }
}

fn get_pk_and_column_macros(fields: &Punctuated<syn::Field, Comma>, ast: &DeriveInput) -> Result<(Pk, Vec<Column>, Vec<Relationship>), syn::Error> {
    let mut pk_column: Option<Pk> = None;
    let mut columns: Vec<Column> = Vec::new();
    let mut relationships: Vec<Relationship> = Vec::new();

    for field in fields.iter() {
        match parse_entity_field(field)? {
            ParsingResult::Column(column) => columns.push(column),
            ParsingResult::Pk(pk) => {
                if pk_column.is_some() {
                    return Err(syn::Error::new(field.span(), "Multiple `#[pk]` columns found; only one is allowed"));
                }
                pk_column = Some(pk);
            },
            ParsingResult::RelationShip(relationship) => relationships.push(relationship),
        }
    }

    let pk_col =
        pk_column.ok_or_else(|| syn::Error::new(ast.span(), "`#[pk]` attribute not found on any column. Exactly one column must have `#[pk]`."))?;

    if columns.is_empty() && relationships.is_empty() {
        return Err(syn::Error::new(ast.span(), "No relationships or #[column(...)] fields found. You must have at least one of those."));
    }

    Ok((pk_col, columns, relationships))
}


#[proc_macro_derive(Entity, attributes(pk, column))]
pub fn derive_entity(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: DeriveInput = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;
    let named_fields = match get_named_fields(&ast) {
        Ok(columns) => columns,
        Err(err) => return err.to_compile_error().into(),
    };
    let (pk_column, columns, relationships) = match get_pk_and_column_macros(&named_fields, &ast) {
        Ok(info) => info,
        Err(err) => return err.to_compile_error().into(),
    };

    let entity_macros = match EntityMacros::new(struct_name.clone(), pk_column, columns, relationships) {
        Ok(struct_macros) => struct_macros,
        Err(err) => return err.to_compile_error().into(),
    };
    proc_macro::TokenStream::from(entity_macros.expand())
}
