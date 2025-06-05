extern crate proc_macro;
mod db_column_macros;
mod entity_macros;
mod db_pk_macros;
mod db_relationship_macros;
mod macro_utils;
mod http_column_macros;
mod http_relationship_macro;
mod http_pk_macros;

use quote::quote;
use crate::entity_macros::EntityMacros;
use crate::db_pk_macros::{DbPkMacros, PointerType};
use syn::{parse_macro_input, DeriveInput};
use syn::spanned::Spanned;

#[proc_macro_derive(PK, attributes(parent))]
pub fn derive_pk(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
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

#[proc_macro_derive(Entity, attributes(pk, column, one2many, one2one, transient))]
pub fn derive_entity(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: DeriveInput = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;

    let register = quote! {
        inventory::submit! {
            EntityInfo {
                name: stringify!(#struct_name),
                routes_fn: #struct_name::routes,
            }
        }
    };

    let stream = EntityMacros::get_named_fields(&ast)
        .and_then(|named_fields| {
            EntityMacros::get_field_macros(&named_fields, &ast)
        })
        .and_then(|field_macros| {
            EntityMacros::new(struct_name.clone(), field_macros)
        })
        .map(|entity_macros| entity_macros.expand()).unwrap_or_else(|e| e.to_compile_error().into());

    // Combine both parts
    let expanded = quote! {
        #stream
        #register
    };

    expanded.into()
}
