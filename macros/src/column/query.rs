use proc_macro2::Ident;
use quote::quote;
use syn::Type;
use crate::entity::query::FilterQueryItem;

pub fn filter_query_init(column_name: &Ident, column_type: &Type) -> FilterQueryItem {
    let definition = quote! { pub #column_name: Option<FilterOp<#column_type>> };
    let init = quote! { #column_name: Some(FilterOp::Eq(#column_type::default())) };
    FilterQueryItem { definition, init }
}