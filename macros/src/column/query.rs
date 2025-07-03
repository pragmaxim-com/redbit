use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

pub fn stream_query_init(column_name: &Ident, column_type: &Type) -> (TokenStream, TokenStream) {
    let definition = quote! { pub #column_name: Option<#column_type> };
    let init = quote! { #column_name: Some(#column_type::default()) };
    (definition, init)
}