use crate::rest::HttpParams::FromPath;
use crate::rest::{FunctionDef, HttpMethod, PathExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;

/// Generates a streaming SSE endpoint definition for querying primary keys by a dictionary index.
pub fn by_dict_def(
    entity_name: &Ident,
    pk_name: &Ident,
    pk_type: &Type,
    column_name: &Ident,
    column_type: &Type,
    value_to_dict_pk: &Ident,
    dict_index_table: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("stream_{}s_by_{}", pk_name, column_name);

    let fn_stream = quote! {
        pub fn #fn_name(
            tx: &ReadTransaction,
            val: &#column_type
        ) -> Result<impl futures::Stream<Item = Result<#pk_type, AppError>> + Send + 'static, AppError> {
            let val2birth = tx.open_table(#value_to_dict_pk)?;
            let birth_guard = val2birth.get(val)?;

            // Box the iterator to unify types
            let iter_box: Box<dyn Iterator<Item = Result<_, _>> + Send> = if let Some(g) = birth_guard {
                let birth_id = g.value().clone();
                let mm = tx.open_multimap_table(#dict_index_table)?;
                let it = mm.get(&birth_id)?;
                Box::new(it)
            } else {
                Box::new(std::iter::empty())
            };

            let stream = stream::iter(iter_box)
                .map(|res| res.map(|e| e.value().clone()).map_err(AppError::from));

            Ok(stream)
        }
    };


    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let pk_stream = #entity_name::#fn_name(&read_tx, &val).expect("Stream creation failed");
            let pks = pk_stream.try_collect::<Vec<#pk_type>>().await.expect("Failed to collect stream");
            assert_eq!(vec![#pk_type::default()], pks);
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let rt = Runtime::new().unwrap();
            let db = DB.clone();
            b.iter(|| {
                rt.block_on(async {
                    let read_tx = db.begin_read().unwrap();
                    let pk_stream = #entity_name::#fn_name(&read_tx, &#column_type::default()).expect("Stream creation failed");
                    pk_stream.try_collect::<Vec<#pk_type>>().await.expect("Failed to collect stream");
                })
            });
        }
    });
    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            entity_name: entity_name.clone(),
            fn_name: fn_name.clone(),
            params: vec![FromPath(vec![PathExpr {
                name: column_name.clone(),
                ty: column_type.clone(),
                description: "Secondary index column (dict)".to_string(),
                sample: quote! { #column_type::default().encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            client_calls: vec![],
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match state.db.begin_read()
                        .map_err(AppError::from)
                        .and_then(|tx| #entity_name::#fn_name(&tx, &#column_name)) {
                            Ok(stream) => axum_streams::StreamBodyAs::json_nl_with_errors(stream).header("Content-Type", HeaderValue::from_str("application/x-ndjson").unwrap()).into_response(),
                            Err(err)   => err.into_response(),
                    }
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/x-ndjson", body = #pk_type),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{}/{{{}}}/{}",
                              entity_name.to_string().to_lowercase(), column_name, column_name, pk_name
            ),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}

/// Generates a streaming SSE endpoint definition for querying primary keys by a simple index.
pub fn by_index_def(
    entity_name: &Ident,
    pk_name: &Ident,
    pk_type: &Type,
    column_name: &Ident,
    column_type: &Type,
    table: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("stream_{}s_by_{}", pk_name, column_name);

    let fn_stream = quote! {
        pub fn #fn_name(
            tx: &ReadTransaction,
            val: &#column_type
        ) -> Result<impl futures::Stream<Item = Result<#pk_type, AppError>> + Send + 'static, AppError> {
            let it = tx.open_multimap_table(#table)?.get(val)?;
            let iter_box: Box<dyn Iterator<Item = Result<_, _>> + Send> = Box::new(it);
            let stream = stream::iter(iter_box).map(|res| res.map(|e| e.value().clone()).map_err(AppError::from));
            Ok(stream)
        }
    };


    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let pk_stream = #entity_name::#fn_name(&read_tx, &val).expect("Stream creation failed");
            let pks = pk_stream.try_collect::<Vec<#pk_type>>().await.expect("Failed to collect stream");
            assert_eq!(vec![#pk_type::default()], pks);
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let rt = Runtime::new().unwrap();
            let db = DB.clone();
            b.iter(|| {
                rt.block_on(async {
                    let read_tx = db.begin_read().unwrap();
                    let pk_stream = #entity_name::#fn_name(&read_tx, &#column_type::default()).expect("Stream creation failed");
                    pk_stream.try_collect::<Vec<#pk_type>>().await.expect("Failed to collect stream");
                })
            });
        }
    });
    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            entity_name: entity_name.clone(),
            fn_name: fn_name.clone(),
            params: vec![FromPath(vec![PathExpr {
                name: column_name.clone(),
                ty: column_type.clone(),
                description: "Secondary index column".to_string(),
                sample: quote! { #column_type::default().encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            client_calls: vec![],
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match state.db.begin_read()
                        .map_err(AppError::from)
                        .and_then(|tx| #entity_name::#fn_name(&tx, &#column_name)) {
                            Ok(stream) => axum_streams::StreamBodyAs::json_nl_with_errors(stream).header("Content-Type", HeaderValue::from_str("application/x-ndjson").unwrap()).into_response(),
                            Err(err)   => err.into_response(),
                    }
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/x-ndjson", body = #pk_type),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{}/{{{}}}/{}",
                              entity_name.to_string().to_lowercase(), column_name, column_name, pk_name
            ),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}
