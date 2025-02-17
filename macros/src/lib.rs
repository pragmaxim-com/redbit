mod column_macros;
mod struct_macros;
mod pk_macros;

extern crate proc_macro;
use proc_macro2::{Ident, Span};
use syn::{parse_macro_input, punctuated::Punctuated, spanned::Spanned, token::Comma, Data, DeriveInput, Field, Fields, Type};
use crate::struct_macros::StructMacros;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Indexing {
    Off,
    On { dictionary: bool, range: bool },
}

enum ParsingResult {
    Pk(Pk),
    Column(Column),
}

struct Pk {
    pk_name: Ident,
    pk_type: Type,
    range: bool,
}

struct Column {
    column_name: Ident,
    column_type: Type,
    indexing: Indexing,
}

fn get_named_fields(ast: &DeriveInput) -> Result<Punctuated<Field, Comma>, syn::Error> {
    match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(columns_named) => Ok(columns_named.named.clone()),
            _ => Err(syn::Error::new(ast.span(), "`#[derive(Redbit)]` only supports structs with named columns.")),
        },
        _ => Err(syn::Error::new(ast.span(), "`#[derive(Redbit)]` can only be applied to structs.")),
    }
}

fn parse_field(field: &Field) -> Result<ParsingResult, syn::Error> {
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
                    column = Some(ParsingResult::Pk(Pk { pk_name: column_name.clone(), pk_type: column_type.clone(), range }));
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
                    column = Some(ParsingResult::Column(Column {
                        column_name: column_name.clone(),
                        column_type: column_type.clone(),
                        indexing: indexing.clone(),
                    }));
                }
            }
            column.ok_or_else(|| syn::Error::new(field.span(), "Field must have either #[pk] or #[column(...)] annotation"))
        }
    }
}

fn get_pk_and_column_macros(fields: &Punctuated<Field, Comma>, span: Span) -> Result<(Pk, Vec<Column>), syn::Error> {
    let mut pk_column: Option<Pk> = None;
    let mut parsed_columns = Vec::new();

    for field in fields.iter() {
        match parse_field(field)? {
            ParsingResult::Column(column) => parsed_columns.push(column),
            ParsingResult::Pk(pk) => {
                if pk_column.is_some() {
                    return Err(syn::Error::new(field.span(), "Multiple `#[pk]` columns found; only one is allowed"));
                }
                pk_column = Some(pk);
            }
        }
    }

    let pk_col =
        pk_column.ok_or_else(|| syn::Error::new(span, "`#[pk]` attribute not found on any column. Exactly one column must have `#[pk]`."))?;

    if parsed_columns.is_empty() {
        return Err(syn::Error::new(span, "No #[column(...)] fields found. You must have at least one field with #[column]."));
    }

    Ok((pk_col, parsed_columns))
}

#[proc_macro_derive(Redbit, attributes(pk, column))]
pub fn derive_indexed(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: DeriveInput = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;
    let named_fields = match get_named_fields(&ast) {
        Ok(columns) => columns,
        Err(err) => return err.to_compile_error().into(),
    };
    let (pk_column, struct_columns) = match get_pk_and_column_macros(&named_fields, ast.span()) {
        Ok(info) => info,
        Err(err) => return err.to_compile_error().into(),
    };

    let struct_macros = match StructMacros::new(struct_columns, struct_name.clone(), pk_column) {
        Ok(struct_macros) => struct_macros,
        Err(err) => return err.to_compile_error().into(),
    };
    proc_macro::TokenStream::from(struct_macros.expand())
}
