use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::FromPath;
use crate::rest::{FunctionDef, HttpMethod, PathExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::macro_utils;

pub fn fn_def(entity_name: &Ident, pk_name: &Ident, pk_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("exists");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &ReadTransaction, pk: &#pk_type) -> Result<bool, AppError> {
            let table_pk_11 = tx.open_table(#table)?;
            if table_pk_11.get(pk)?.is_some() {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let pk_value = #pk_type::default();
            let entity_exists = #entity_name::#fn_name(&read_tx, &pk_value).expect("Failed to check entity exists");
            assert!(entity_exists, "Entity is supposed to exist for the given PK");
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
                #entity_name::#fn_name(&read_tx, &pk_value).expect("Failed to check entity exists");
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
            method: HttpMethod::HEAD,
            handler_name: format_ident!("{}", handler_fn_name),
            utoipa_responses: quote! { responses((status = OK)) },
            handler_impl_stream: quote! {
                Result<axum::http::StatusCode, AppError> {
                    let tx = state.db.begin_read().map_err(AppError::from)?;
                    match #entity_name::#fn_name(&tx, &#pk_name) {
                        Ok(true) => Ok(axum::http::StatusCode::OK),
                        Ok(false) => Ok(axum::http::StatusCode::NOT_FOUND),
                        Err(e) => Err(AppError::from(e)),
                    }
                }
            },
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), pk_name, pk_name),
        }),
        test_stream,
        bench_stream
    }
}
