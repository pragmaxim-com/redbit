use crate::rest::HttpParams::FromPath;
use crate::rest::{FunctionDef, HttpMethod, PathExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;
use crate::macro_utils;

pub fn fn_def(entity_name: &Ident, pk_name: &Ident, pk_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("parent_key");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &ReadTransaction, pk: &#pk_type) -> Result<<#pk_type as ChildPointer>::Parent, AppError> {
            Ok(pk.parent().clone())
        }
    };
    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            params: vec![FromPath(vec![PathExpr {
                name: pk_name.clone(),
                ty: pk_type.clone(),
                description: "Primary key of the owner entity".to_string(),
                sample: quote! { #pk_type::default().encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            client_call: Some(macro_utils::client_code(&handler_fn_name, pk_type, pk_name)),
            handler_impl_stream: quote! {
               Result<AppJson<<#pk_type as ChildPointer>::Parent>, AppError> {
                    state.db.begin_read().map_err(AppError::from).and_then(|tx| #entity_name::#fn_name(&tx, &#pk_name)).map(AppJson)
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/json", body = #pk_type),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{}/{{{}}}/{}", entity_name.to_string().to_lowercase(), pk_name, pk_name, fn_name),
        }),
        test_stream: None,
        bench_stream: None
    }
}
