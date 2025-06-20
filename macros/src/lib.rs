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
use quote::quote;
use std::sync::Once;
use syn::parse::Parse;
use syn::spanned::Spanned;
use syn::{parse_macro_input, parse_quote, DeriveInput, Fields, ItemStruct, Type};

#[proc_macro_attribute]
pub fn indexed_column(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemStruct);
    let struct_ident = &input.ident;

    let field_ty = match &input.fields {
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => &fields.unnamed[0].ty,
        _ => {
            return quote! {
                compile_error!("`#[column]` can only be used on tuple structs with a single field.");
                #input
            }
                .into();
        }
    };

    // Clean any existing derives and insert our own
    input.attrs.retain(|a| !a.path().is_ident("derive"));
    input.attrs.insert(0, syn::parse_quote! {
        #[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Debug, serde::Serialize, serde::Deserialize, redbit::utoipa::ToSchema)]
    });

    // Determine default value based on inner type
    let default_expr = match quote!(#field_ty).to_string().as_str() {
        "String" | "std :: string :: String" => {
            quote! { Self("default-value".to_string()) }
        },
        "&str" => {
            quote! { Self("default-value".into()) }
        },
        _ => {
            quote! { Self(Default::default()) }
        },
    };

    let display_impl = quote! {
        impl std::fmt::Display for #struct_ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };

    let default_impl = quote! {
        impl Default for #struct_ident {
            fn default() -> Self {
                #default_expr
            }
        }
    };

    let expanded = quote! {
        #input
        #default_impl
        #display_impl
    };

    expanded.into()
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
pub fn primary_key(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut s = parse_macro_input!(item as ItemStruct);

    s.attrs.retain(|a| !a.path().is_ident("derive"));
    s.attrs.insert(0, parse_quote! {
        #[derive(Pk, Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
    });
    quote!(#s).into()
}

#[proc_macro_attribute]
pub fn foreign_key(attr: TokenStream, item: TokenStream) -> TokenStream {
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
        #[derive(Fk, Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
        #vis struct #struct_ident {
            pub parent: #parent_type,
            pub index: #index_type,
        }
    };

    expanded.into()
}

#[proc_macro_derive(Fk)]
pub fn derive_fk(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;

    match DbPkMacros::validate_fk(&ast) {
        Ok(_) => {
            let (parent_field, index_field) = match DbPkMacros::extract_fields(&ast, &PointerType::Child) {
                Ok(fields) => fields,
                Err(e) => return e.to_compile_error().into(),
            };
            match parent_field {
                Some(parent_field) => DbPkMacros::generate_child_impls(struct_name, parent_field, index_field).into(),
                None => syn::Error::new(index_field.span(), "Parent field missing").to_compile_error().into(),
            }
        },
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(Pk)]
pub fn derive_pk(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;

    match DbPkMacros::validate_pk(&ast) {
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
