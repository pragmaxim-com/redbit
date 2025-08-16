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

use crate::pk::PointerType;
use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use quote::quote;
use syn::parse::Parse;
use syn::spanned::Spanned;
use syn::{parse_macro_input, parse_quote, DeriveInput, Fields, ItemStruct, Lit, Path, Type};
use syn::punctuated::Punctuated;
use syn::token::Comma;

#[proc_macro_attribute]
#[proc_macro_error]
pub fn column(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_args = parse_macro_input!(attr as EncodingAttr);
    let mut input = parse_macro_input!(item as ItemStruct);
    let struct_ident = &input.ident.clone();
    let stream =
        match &mut input.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let (impls, maybe_field_attr, extra_derive_impls) =
                    column::impls::generate_column_impls(struct_ident, &fields.unnamed[0].ty, attr_args.encoding);

                if let Some(attr) = maybe_field_attr {
                    input.attrs.push(syn::parse_quote! { #[serde_with::serde_as] });
                    fields.unnamed[0].attrs.push(attr);
                }

                let mut derives: Punctuated<Path, Comma> = syn::parse_quote![Clone, Hash, Eq, Ord, PartialEq, PartialOrd, Debug, Decode, Encode, Serialize, Deserialize];
                derives.extend(extra_derive_impls);
                macro_utils::merge_struct_derives(&mut input, derives);
                quote! {
                    #input
                    #impls
                }
            },
            _ => {
                let derives: Punctuated<Path, Comma> = syn::parse_quote![Decode, Encode, Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq, utoipa::ToSchema];
                macro_utils::merge_struct_derives(&mut input, derives);
                quote! {
                    #input
                }
            }
        };

    macro_utils::submit_struct_to_stream(stream, "column", struct_ident, ".rs")
}

struct EncodingAttr {
    encoding: Option<String>,
}

impl Parse for EncodingAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            Ok(EncodingAttr { encoding: None })
        } else {
            let literal: Lit = input.parse()?;
            match literal {
                Lit::Str(lit_str) => Ok(EncodingAttr {
                    encoding: Some(lit_str.value()), // unquoted, unescaped
                }),
                _ => Err(syn::Error::new_spanned(literal, "Expected a string literal")),
            }
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

    let stream = quote! {
        #[derive(PointerKey, Copy, Clone, Debug, Default, Decode, Encode, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(into = "String", try_from = "String")]
        #vis struct #struct_ident {
            pub parent: #parent_type,
            pub index: #index_type,
        }
    };
    macro_utils::submit_struct_to_stream(stream, "pk", struct_ident, "_attr.rs")
}

#[proc_macro_derive(PointerKey)]
#[proc_macro_error]
pub fn derive_pointer_key(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_ident = &ast.ident;

    let stream = match field_parser::validate_pointer_key(&ast) {
        Ok(_) => {
            let (parent_field, index_field) = match field_parser::extract_pointer_key_fields(&ast, &PointerType::Child) {
                Ok(fields) => fields,
                Err(e) => return e.to_compile_error().into(),
            };
            match parent_field {
                Some(parent_field) => pk::pointer_impls::new(struct_ident, parent_field, index_field),
                None => syn::Error::new(index_field.span(), "Parent field missing").to_compile_error(),
            }
        },
        #[allow(clippy::useless_conversion)]
        Err(e) => e.to_compile_error().into(),
    };
    macro_utils::submit_struct_to_stream(stream, "pk", struct_ident, "_derive.rs")
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn root_key(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut s = parse_macro_input!(item as ItemStruct);
    let struct_ident = &s.ident;
    s.attrs.retain(|a| !a.path().is_ident("derive"));
    s.attrs.insert(0, parse_quote! {
        #[derive(RootKey, Copy, Clone, Hash, Debug, Decode, Encode, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
    });
    let stream = quote!(#s);
    macro_utils::submit_struct_to_stream(stream, "pk", struct_ident, "_attr.rs")
}

#[proc_macro_derive(RootKey)]
#[proc_macro_error]
pub fn derive_root_key(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_ident = &ast.ident;

    #[allow(clippy::useless_conversion)]
    let stream: proc_macro2::TokenStream = match field_parser::validate_root_key(&ast) {
        Ok(_) => {
            let (_, index_field) = match field_parser::extract_pointer_key_fields(&ast, &PointerType::Root) {
                Ok(fields) => fields,
                Err(e) => return e.to_compile_error().into(),
            };
            pk::root_impls::new(struct_ident, index_field)
        },
        Err(e) => e.to_compile_error().into(),
    };
    macro_utils::submit_struct_to_stream(stream, "pk", struct_ident, "_derive.rs")
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn entity(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut s = parse_macro_input!(item as ItemStruct);
    let struct_ident = &s.ident;
    s.attrs.retain(|a| !a.path().is_ident("derive"));
    s.attrs.insert(0, parse_quote! {
        #[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, Entity, PartialEq, ToSchema)]
    });
    let stream = quote! {
        #s
    };
    macro_utils::submit_struct_to_stream(stream, "entity", struct_ident, "_attr.rs")
}

#[proc_macro_derive(Entity, attributes(pk, fk, column))]
#[proc_macro_error]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    let item_struct = parse_macro_input!(input as ItemStruct);
    let struct_ident = &item_struct.ident;
    let (key_def, s) = match entity::new(&item_struct) {
        Ok(result) => result,
        Err(e) => return e.to_compile_error().into(),
    };
    let root = key_def.is_root();
    let stream = quote! {
        #s
        inventory::submit! {
            StructInfo {
                name: stringify!(#struct_ident),
                root: #root,
                routes_fn: #struct_ident::routes,
            }
        }
    };

    macro_utils::submit_struct_to_stream(stream, "entity", &item_struct.ident, "_derive.rs")
}
