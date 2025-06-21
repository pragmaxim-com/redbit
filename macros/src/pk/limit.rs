use crate::rest::HttpParams::FromQuery;
use crate::rest::{EndpointDef, FunctionDef, HttpMethod};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn limit_fn_def(entity_name: &Ident, entity_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("limit");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx: &::redbit::redb::ReadTransaction, query: LimitQuery) -> Result<Vec<#entity_type>, AppError> {
                match query {
                    LimitQuery{take: Some(n), ..} => {
                        #entity_name::take(&tx, n)
                    },
                    LimitQuery{first: Some(true), ..} => {
                        #entity_name::first(&tx).map(|r| r.into_iter().collect())
                    },
                    LimitQuery{last: Some(true), ..} => {
                        #entity_name::last(&tx).map(|r| r.into_iter().collect())
                    },
                    LimitQuery{..} => {
                        panic!("LimitQuery must have at least one of take, first, or last defined");
                    }
                }
            }
        };

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(Vec<#entity_type>),
        fn_stream,
        fn_call: quote! { #entity_name::#fn_name(&tx, query) },
        endpoint_def: Some(EndpointDef {
            params: FromQuery(syn::parse_quote!(LimitQuery)),
            method: HttpMethod::GET,
            return_type: Some(syn::parse_quote!(Vec<#entity_type>)),
            endpoint: format!("/{}", entity_name.to_string().to_lowercase()),
        }),
        test_stream: None
    }
}
