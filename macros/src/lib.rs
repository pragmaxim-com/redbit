extern crate proc_macro;
mod column;
mod entity;
mod pk;
mod relationship;
mod macro_utils;
mod rest;
mod field_parser;
mod compositor;
mod table;
mod transient;

use crate::entity::EntityMacros;
use crate::pk::{DbPkMacros, PointerType};

use crate::column::DbColumnMacros;
use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use quote::quote;
use std::sync::Once;
use syn::parse::Parse;
use syn::spanned::Spanned;
use syn::{parse_macro_input, parse_quote, DeriveInput, Fields, ItemStruct, Type};

#[proc_macro_attribute]
#[proc_macro_error]
pub fn column(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemStruct);
    let struct_ident = &input.ident.clone();

    match &input.clone().fields {
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            macro_utils::merge_struct_derives(&mut input, syn::parse_quote![Clone, Eq, Ord, PartialEq, PartialOrd, Debug]);
            DbColumnMacros::generate_column_impls(struct_ident, &input, &fields.unnamed[0].ty).into()
        },
        _ => {
            macro_utils::merge_struct_derives(&mut input, syn::parse_quote![Serialize, Deserialize, Debug, Clone, PartialEq, Eq, utoipa::ToSchema]);
            quote! {
                #input
            }.into()
        }
    }
}

struct KeyAttr {
    index_type: Option<Type>,
}

impl Parse for KeyAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            Ok(KeyAttr { index_type: None })
        } else {
            let ty: Type = input.parse()?;
            Ok(KeyAttr { index_type: Some(ty) })
        }
    }
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn pointer_key(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_args = parse_macro_input!(attr as KeyAttr);
    let s = parse_macro_input!(item as ItemStruct);

    let struct_ident = &s.ident;
    let vis = &s.vis;
    let index_type = attr_args.index_type.unwrap_or_else(|| syn::parse_quote! { u16 });

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
        #[derive(PointerKey, Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
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

    match DbPkMacros::validate_pointer_key(&ast) {
        Ok(_) => {
            let (parent_field, index_field) = match DbPkMacros::extract_fields(&ast, &PointerType::Child) {
                Ok(fields) => fields,
                Err(e) => return e.to_compile_error().into(),
            };
            match parent_field {
                Some(parent_field) => DbPkMacros::generate_pointer_impls(struct_name, parent_field, index_field).into(),
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
        #[derive(RootKey, Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
    });
    quote!(#s).into()
}

#[proc_macro_derive(RootKey)]
#[proc_macro_error]
pub fn derive_root_key(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;

    match DbPkMacros::validate_root_key(&ast) {
        Ok(_) => {
            let (_, index_field) = match DbPkMacros::extract_fields(&ast, &PointerType::Root) {
                Ok(fields) => fields,
                Err(e) => return e.to_compile_error().into(),
            };
            DbPkMacros::generate_root_impls(struct_name, index_field).into()
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

    let stream = field_parser::get_named_fields(&item_struct)
        .and_then(|named_fields| {
            field_parser::get_field_macros(&named_fields, &item_struct)
        })
        .and_then(|field_macros| {
            EntityMacros::new(entity_ident, &entity_type, field_macros)
        })
        .map(|entity_macros| compositor::expand(entity_macros)).unwrap_or_else(|e| e.to_compile_error().into());

    // Combine both parts
    let expanded = quote! {
        #stream
        #register
    };

    expanded.into()
}
