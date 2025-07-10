use crate::rest::HttpParams::FromQuery;
use crate::rest::{FunctionDef, HttpMethod, Param};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;

pub fn limit_fn_def(entity_name: &Ident, entity_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("limit");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx: &ReadTransaction, query: LimitQuery) -> Result<Vec<#entity_type>, AppError> {
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
                        Err(AppError::BadRequest("LimitQuery must have at least one of take, first, or last defined".to_string()))
                    }
                }
            }
        };

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            params: vec![FromQuery(Param {
                name: format_ident!("query"), // TODO
                ty: syn::parse_quote!(LimitQuery),
                description: "Query parameter for limiting results".to_string(),
                samples: quote! { vec![LimitQuery::sample()] }, // TODO many
            })],
            method: HttpMethod::GET,
            handler_impl_stream: quote! {
               Result<AppJson<Vec<#entity_type>>, AppError> {
                    state.db.begin_read().map_err(AppError::from).and_then(|tx| #entity_name::#fn_name(&tx, query)).map(AppJson)
                }
            },
            utoipa_responses: quote! { responses((status = OK, body = Vec<#entity_type>)) },
            endpoint: format!("/{}", entity_name.to_string().to_lowercase()),
        }),
        test_stream: None,
        bench_stream: None
    }
}
