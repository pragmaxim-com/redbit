use crate::entity::query::FilterQueryItem;
use proc_macro2::Ident;
use quote::quote;
use syn::Type;

pub fn query_init(child_name: &Ident, child_stream_query_type: &Type) -> FilterQueryItem {
    let definition = quote! { pub #child_name: Option<#child_stream_query_type> };
    let init = quote! { #child_name: Some(#child_stream_query_type::sample()) };
    FilterQueryItem { definition, init }
}
