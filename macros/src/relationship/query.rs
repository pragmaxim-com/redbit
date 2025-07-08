use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

pub fn stream_query_init(child_name: &Ident, child_stream_query_type: &Type) -> (TokenStream, TokenStream) {
    let definition = quote! { pub #child_name: Option<#child_stream_query_type> };
    let init = quote! { #child_name: Some(#child_stream_query_type::sample()) };
    (definition, init)
}
