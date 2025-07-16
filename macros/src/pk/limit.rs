use crate::rest::HttpParams::FromQuery;
use crate::rest::{FunctionDef, HttpMethod, QueryExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;
use crate::macro_utils;

pub fn limit_fn_def(entity_name: &Ident, entity_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("limit");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx: &ReadTransaction, query: LimitQuery) -> Result<Vec<#entity_type>, AppError> {
                match query {
                    LimitQuery{take: Some(n), ..} => {
                        #entity_name::take(&tx, n)
                    },
                    LimitQuery{tail: Some(n), ..} => {
                        #entity_name::tail(&tx, n)
                    },
                    LimitQuery{first: Some(true), ..} => {
                        #entity_name::first(&tx).map(|r| r.into_iter().collect())
                    },
                    LimitQuery{last: Some(true), ..} => {
                        #entity_name::last(&tx).map(|r| r.into_iter().collect())
                    },
                    LimitQuery{..} => {
                        Err(AppError::BadRequest("LimitQuery must have one of take, tail, first, or last defined".to_string()))
                    }
                }
            }
        };

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);
    fn client_call(query: &str, handler_fn_name: &str) -> String {
        format!(
    r#"
    it("it makes {function_name} with query {query}", async () => {{
        client.{function_name}({{
            query: {{
                {query}
            }},
            throwOnError: false
        }}).then(function({{data, request, response, error}}) {{
            console.log("{function_name} with {query} succeeded with response: ", response.status, error?.message, data);
        }}).catch(function({{message}}) {{
            console.error("{function_name} with {query} failed with error :", message);
        }});
    }});
    "#,
    function_name = format_ident!("{}", macro_utils::to_camel_case(handler_fn_name, false)),
    )
    }

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            entity_name: entity_name.clone(),
            fn_name: fn_name.clone(),
            params: vec![FromQuery(QueryExpr {
                ty: syn::parse_quote!(LimitQuery),
                extraction: quote! { extract::Query(query): extract::Query<LimitQuery> },
                samples: quote! { vec![LimitQuery::sample()] }, // TODO many
            })],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            client_calls: vec![
                client_call("take: 2", &handler_fn_name),
                client_call("tail: 2", &handler_fn_name),
                client_call("first: true", &handler_fn_name),
                client_call("last: true", &handler_fn_name),
            ],
            handler_impl_stream: quote! {
               Result<AppJson<Vec<#entity_type>>, AppError> {
                    state.db.begin_read().map_err(AppError::from).and_then(|tx| #entity_name::#fn_name(&tx, query)).map(AppJson)
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/json", body = Vec<#entity_type>),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}", entity_name.to_string().to_lowercase()),
        }.to_endpoint()),
        test_stream: None,
        bench_stream: None
    }
}
