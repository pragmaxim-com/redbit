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

use crate::entity::EntityMacros;
use crate::pk::{DbPkMacros, PointerType};

use proc_macro::TokenStream;
use std::sync::Once;
use quote::quote;
use syn::{parse_macro_input, parse_quote, DeriveInput, ItemStruct};
use syn::spanned::Spanned;


#[proc_macro_attribute]
pub fn key(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut s = parse_macro_input!(item as ItemStruct);
    s.attrs.retain(|a| !a.path().is_ident("derive"));
    s.attrs.insert(0, parse_quote! {
        #[derive(Clone, Debug, Default, Eq, Ord, Pk, PartialEq, PartialOrd)]
    });
    quote!(#s).into()
}


#[proc_macro_derive(Pk, attributes(parent))]
pub fn derive_pk(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;

    let pointer_type = match DbPkMacros::extract_pointer_type(&ast) {
        Ok(t) => t,
        Err(e) => return e.to_compile_error().into(),
    };

    let (parent_field, index_field) = match DbPkMacros::extract_fields(&ast, &pointer_type) {
        Ok(fields) => fields,
        Err(e) => return e.to_compile_error().into(),
    };

    match pointer_type {
        PointerType::Root => DbPkMacros::generate_root_impls(struct_name, index_field).into(),
        PointerType::Child =>
            match parent_field {
                Some(parent_field) => DbPkMacros::generate_child_impls(struct_name, parent_field, index_field).into(),
                None => syn::Error::new(index_field.span(), "Parent field missing").to_compile_error().into(),
            }
    }
}

#[proc_macro_attribute]
pub fn entity(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut s = parse_macro_input!(item as ItemStruct);
    s.attrs.retain(|a| !a.path().is_ident("derive"));
    s.attrs.insert(0, parse_quote! {
        #[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, Eq, Entity, PartialEq, redbit::utoipa::ToSchema)]
    });
    quote!(#s).into()
}

#[proc_macro_derive(Entity, attributes(pk, fk, column, one2many, one2one, transient))]
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
