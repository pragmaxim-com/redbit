use crate::endpoint::EndpointDef;
use crate::macro_utils;
use crate::rest::HttpParams::FromPath;
use crate::rest::{FunctionDef, HttpMethod, PathExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("get");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &ReadTransaction, pk: &#pk_type) -> Result<Option<#entity_type>, AppError> {
            let table_pk_5 = tx.open_table(#table)?;
            if table_pk_5.get(pk)?.is_some() {
                Ok(Some(Self::compose(&tx, pk)?))
            } else {
                Ok(None)
            }
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let pk_value = #pk_type::default();
            let entity = #entity_name::#fn_name(&read_tx, &pk_value).expect("Failed to get entity by PK").expect("Expected entity to exist");
            let expected_enity = #entity_type::sample();
            assert_eq!(entity, expected_enity, "Entity PK does not match the requested PK");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let pk_value = #pk_type::default();
            b.iter(|| {
                #entity_name::#fn_name(&read_tx, &pk_value).expect("Failed to get entity by PK").expect("Expected entity to exist");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            params: vec![FromPath(vec![PathExpr {
                name: pk_name.clone(),
                ty: pk_type.clone(),
                description: "Primary key".to_string(),
                sample: quote! { #pk_type::default().encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            client_call: Some(macro_utils::client_code(&handler_fn_name, pk_type, pk_name)),
            handler_impl_stream: quote! {
              impl IntoResponse {
                   match state.db.begin_read()
                   .map_err(AppError::from)
                   .and_then(|tx| #entity_name::#fn_name(&tx, &#pk_name) ) {
                       Ok(Some(entity)) => {
                           (StatusCode::OK, AppJson(entity)).into_response()
                       },
                       Ok(None) => {
                            let message = format!("{} not found", stringify!(#entity_name));
                            let response = ErrorResponse { message, code: StatusCode::NOT_FOUND.as_u16() };
                            (StatusCode::NOT_FOUND, AppJson(response)).into_response()
                       },
                       Err(err) => err.into_response(),
                   }
               }
            },
            utoipa_responses: quote! { 
                responses(
                    (status = OK, content_type = "application/json", body = #entity_type), 
                    (status = NOT_FOUND, content_type = "application/json", body = ErrorResponse)
                ) 
            },
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), pk_name, pk_name),
        }),
        test_stream,
        bench_stream,
    }
}
