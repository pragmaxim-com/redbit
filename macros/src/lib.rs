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

use syn::{punctuated::Punctuated, Path, token::Comma};
use syn::{Attribute, Result};
use proc_macro::TokenStream;
use quote::quote;
use std::sync::Once;
use proc_macro_error::proc_macro_error;
use syn::parse::Parse;
use syn::spanned::Spanned;
use syn::{parse_macro_input, parse_quote, DeriveInput, Fields, ItemStruct, Type};
use crate::column::DbColumnMacros;

#[proc_macro_attribute]
#[proc_macro_error]
pub fn index(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemStruct);
    let struct_ident = &input.ident;

    let field_ty = match &input.fields {
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => &fields.unnamed[0].ty,
        _ => {
            return quote! {
                compile_error!("`#[index]` can only be used on tuple structs with a single field.");
                #input
            }.into();
        }
    };

    // Collect derives from existing #[derive(...)] attributes
    let mut existing_derives = Vec::<syn::Path>::new();

    fn extract_derives(attr: &Attribute) -> Result<Vec<syn::Path>> {
        let mut derives = Vec::new();
        attr.parse_nested_meta(|meta| {
            if let Some(ident) = meta.path.get_ident() {
                derives.push(syn::Path::from(ident.clone()));
                Ok(())
            } else {
                Err(meta.error("Expected identifier in derive"))
            }
        })?;
        Ok(derives)
    }

    input.attrs.retain(|attr| {
        if attr.path().is_ident("derive") {
            // parse_nested_meta applies to the content inside #[derive(...)]
            match extract_derives(attr) {
                Ok(paths) => existing_derives.extend(paths),
                Err(e) => {
                    eprintln!("Error parsing derive attribute: {}", e);
                    return true; // keep the attribute to avoid losing it
                }
            }
            false // remove this derive attr, we'll reinsert merged one later
        } else {
            true
        }
    });

    let extra_derives: Punctuated<Path, Comma> = syn::parse_quote![Clone, Eq, Ord, PartialEq, PartialOrd, Debug];
    let extra_derives_vec: Vec<Path> = extra_derives.into_iter().collect();
    // Merge, deduplicate
    existing_derives.extend(extra_derives_vec);
    existing_derives.sort_by(|a, b| quote!(#a).to_string().cmp(&quote!(#b).to_string()));
    existing_derives.dedup_by(|a, b| quote!(#a).to_string() == quote!(#b).to_string());

    // Reinsert merged derive attribute
    input.attrs.push(syn::parse_quote! {
        #[derive(#(#existing_derives),*)]
    });

    DbColumnMacros::generate_index_impls(struct_ident, &input, field_ty).into()
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
        #[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, Eq, Entity, PartialEq, redbit::utoipa::ToSchema)]
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
