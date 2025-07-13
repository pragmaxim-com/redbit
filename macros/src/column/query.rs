use proc_macro2::Ident;
use quote::quote;
use syn::Type;
use crate::entity::query::StreamQueryItem;

pub fn stream_query_init(column_name: &Ident, column_type: &Type) -> StreamQueryItem {
    let definition = quote! { pub #column_name: Option<FilterOp<#column_type>> };
    let init = quote! { #column_name: Some(FilterOp::Eq(#column_type::default())) };
    StreamQueryItem { definition, init }
}