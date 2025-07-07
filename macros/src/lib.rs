extern crate proc_macro;
mod column;
mod pk;
mod relationship;
mod macro_utils;
mod rest;
mod field_parser;
mod table;
mod transient;
mod endpoint;
mod field;
mod entity;

use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use quote::{format_ident, quote};
use std::sync::Once;
use proc_macro2::Literal;
use syn::parse::Parse;
use syn::spanned::Spanned;
use syn::{parse_macro_input, parse_quote, DeriveInput, Fields, ItemStruct, Type};
use crate::column::DbColumnMacros;
use crate::relationship::DbRelationshipMacros;
use crate::pk::{PointerType, DbPkMacros};
use crate::field::FieldMacros;
use crate::field_parser::ColumnDef;
use crate::transient::TransientMacros;

#[proc_macro_attribute]
#[proc_macro_error]
pub fn column(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_args = parse_macro_input!(attr as EncodingAttr);
    let mut input = parse_macro_input!(item as ItemStruct);
    let struct_ident = &input.ident.clone();
    let expanded =
        match &mut input.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let (impls, maybe_field_attr) =
                    column::impls::generate_column_impls(struct_ident, &fields.unnamed[0].ty, attr_args.literal);

                if let Some(attr) = maybe_field_attr {
                    input.attrs.push(syn::parse_quote! { #[serde_with::serde_as] });
                    fields.unnamed[0].attrs.push(attr);
                }

                macro_utils::merge_struct_derives(&mut input, syn::parse_quote![Clone, Eq, Ord, PartialEq, PartialOrd, Debug, Serialize, Deserialize]);
                quote! {
                    #input
                    #impls
                }.into()
            },
            _ => {
                macro_utils::merge_struct_derives(&mut input, syn::parse_quote![
                    Serialize, Deserialize, Debug, Clone, PartialEq, Eq, utoipa::ToSchema
                ]);
                quote! {
                    #input
                }.into()
            }
        };
    macro_utils::write_stream_and_return(expanded, struct_ident).into()
}

struct EncodingAttr {
    literal: Option<Literal>,
}

impl Parse for EncodingAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            Ok(EncodingAttr { literal: None })
        } else {
            let literal: Literal = input.parse()?;
            Ok(EncodingAttr { literal: Some(literal) })
        }
    }
}

struct IndexTypeAttr {
    tpe: Option<Type>,
}

impl Parse for IndexTypeAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            Ok(IndexTypeAttr { tpe: None })
        } else {
            let ty: Type = input.parse()?;
            Ok(IndexTypeAttr { tpe: Some(ty) })
        }
    }
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn pointer_key(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_args = parse_macro_input!(attr as IndexTypeAttr);
    let s = parse_macro_input!(item as ItemStruct);

    let struct_ident = &s.ident;
    let vis = &s.vis;
    let index_type = attr_args.tpe.unwrap_or_else(|| syn::parse_quote! { u16 });

    // Validate tuple struct with one field
    let parent_type = match &s.fields {
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            &fields.unnamed[0].ty
        }
        _ => {
            return syn::Error::new_spanned(
                &s.ident,
                "#[foreign_key] must be applied to a tuple struct with one field (the parent)"
            )
                .to_compile_error()
                .into();
        }
    };

    let expanded = quote! {
        #[derive(PointerKey, Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(into = "String", try_from = "String")]
        #vis struct #struct_ident {
            pub parent: #parent_type,
            pub index: #index_type,
        }
    };

    expanded.into()
}

#[proc_macro_derive(PointerKey)]
#[proc_macro_error]
pub fn derive_pointer_key(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;

    match field_parser::validate_pointer_key(&ast) {
        Ok(_) => {
            let (parent_field, index_field) = match field_parser::extract_pointer_key_fields(&ast, &PointerType::Child) {
                Ok(fields) => fields,
                Err(e) => return e.to_compile_error().into(),
            };
            match parent_field {
                Some(parent_field) => pk::pointer_impls::new(struct_name, parent_field, index_field).into(),
                None => syn::Error::new(index_field.span(), "Parent field missing").to_compile_error().into(),
            }
        },
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn root_key(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut s = parse_macro_input!(item as ItemStruct);

    s.attrs.retain(|a| !a.path().is_ident("derive"));
    s.attrs.insert(0, parse_quote! {
        #[derive(RootKey, Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
    });
    quote!(#s).into()
}

#[proc_macro_derive(RootKey)]
#[proc_macro_error]
pub fn derive_root_key(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;

    match field_parser::validate_root_key(&ast) {
        Ok(_) => {
            let (_, index_field) = match field_parser::extract_pointer_key_fields(&ast, &PointerType::Root) {
                Ok(fields) => fields,
                Err(e) => return e.to_compile_error().into(),
            };
            pk::root_impls::new(struct_name, index_field).into()
        },
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn entity(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut s = parse_macro_input!(item as ItemStruct);
    s.attrs.retain(|a| !a.path().is_ident("derive"));
    s.attrs.insert(0, parse_quote! {
        #[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, Entity, PartialEq, ToSchema)]
    });
    quote!(#s).into()
}

#[proc_macro_derive(Entity, attributes(pk, fk, column))]
#[proc_macro_error]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    let item_struct = parse_macro_input!(input as ItemStruct);
    let entity_ident = &item_struct.ident;
    let entity_type: syn::Type = parse_quote! { #entity_ident };

    static PRINT_ONCE: Once = Once::new();
    PRINT_ONCE.call_once(|| {
        // eprintln!("----------------------------------------------------------");
    });

    let register = quote! {
        inventory::submit! {
            EntityInfo {
                name: stringify!(#entity_ident),
                routes_fn: #entity_ident::routes,
            }
        }
    };
    let stream_query_ident = format_ident!("{}StreamQuery", entity_ident);
    let stream_query_type: Type = syn::parse_quote! { #stream_query_ident };

    let stream = field_parser::get_named_fields(&item_struct)
        .and_then(|named_fields| {
            field_parser::get_field_macros(&named_fields, &item_struct)
        })
        .and_then(|(pk, field_macros)| {
            let field_macros =
                field_macros.into_iter().map(|c| match c {
                    ColumnDef::Key {field_def, fk } => {
                        FieldMacros::Pk(DbPkMacros::new(entity_ident, &entity_type, field_def.clone(), fk.clone()))
                    },
                    ColumnDef::Plain(field , indexing_type) => {
                        FieldMacros::Plain(DbColumnMacros::new(field.clone(), indexing_type.clone(), entity_ident, &entity_type, &pk.name, &pk.tpe, &stream_query_type))
                    },
                    ColumnDef::Relationship(field, multiplicity) => {
                        FieldMacros::Relationship(DbRelationshipMacros::new(field.clone(), multiplicity.clone(), entity_ident, &pk.name, &pk.tpe))
                    },
                    ColumnDef::Transient(field) =>{
                        FieldMacros::Transient(TransientMacros::new(field.clone()))
                    }
                }
                ).collect::<Vec<FieldMacros>>();
            entity::EntityMacros::new(entity_ident.clone(), entity_type, pk.name, pk.tpe, &stream_query_type, field_macros)
        })
        .map(|entity_macros| entity_macros.expand()).unwrap_or_else(|e| e.to_compile_error().into());

    // Combine both parts
    let expanded = quote! {
        #stream
        #register
    };

    expanded.into()
}
