use crate::rest::HttpParams::Path;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod, PathExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;
use crate::table::DictTableDefs;

/// Generates a streaming SSE endpoint definition for querying primary keys by a dictionary index.
pub fn by_dict_def(
    entity_name: &Ident,
    pk_name: &Ident,
    pk_type: &Type,
    column_name: &Ident,
    column_type: &Type,
    tx_context_ty: &Type,
    dict_table_defs: &DictTableDefs,
) -> FunctionDef {
    let dict_table_var = &dict_table_defs.var_name;
    let fn_name = format_ident!("stream_{}s_by_{}", pk_name, column_name);
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: #tx_context_ty, val: #column_type) -> Result<impl futures::Stream<Item = Result<#pk_type, AppError>> + Send, AppError> {
            let multi_value = tx_context.#dict_table_var.get_keys(val)?;
            let iter_box: Box<dyn Iterator<Item = Result<_, _>> + Send> =
                if let Some(v) = multi_value {
                    Box::new(v)
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
            let storage = STORAGE.clone();
            let val = #column_type::default();
            let read_tx = storage.db.begin_read().expect("Failed to begin read transaction");
            let tx_context = #entity_name::begin_read_tx(&read_tx).expect("Failed to begin read transaction context");
            let pk_stream = #entity_name::#fn_name(tx_context, val).expect("Stream creation failed");
            let pks = pk_stream.try_collect::<Vec<#pk_type>>().await.expect("Failed to collect stream");
            assert_eq!(vec![#pk_type::default()], pks);
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let rt = Runtime::new().unwrap();
            let storage = STORAGE.clone();
            b.iter(|| {
                rt.block_on(async {
                    let read_tx = storage.db.begin_read().unwrap();
                    let tx_context = #entity_name::begin_read_tx(&read_tx).expect("Failed to begin read transaction context");
                    let pk_stream = #entity_name::#fn_name(tx_context, #column_type::default()).expect("Stream creation failed");
                    pk_stream.try_collect::<Vec<#pk_type>>().await.expect("Failed to collect stream");
                })
            });
        }
    });
    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            _entity_name: entity_name.clone(),
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![Path(vec![PathExpr {
                name: column_name.clone(),
                ty: column_type.clone(),
                description: "Secondary index column (dict)".to_string(),
                sample: quote! { #column_type::default().url_encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match state.storage.db.begin_read()
                        .map_err(AppError::from)
                        .and_then(|tx| #entity_name::begin_read_tx(&tx).map_err(AppError::from))
                        .and_then(|tx_context| #entity_name::#fn_name(tx_context, #column_name)) {
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
    tx_context_ty: &Type,
    table: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("stream_{}s_by_{}", pk_name, column_name);

    let fn_stream = quote! {
        pub fn #fn_name(tx_context: #tx_context_ty, val: #column_type) -> Result<impl futures::Stream<Item = Result<#pk_type, AppError>> + Send, AppError> {
            let it = tx_context.#table.get(val)?;
            let iter_box: Box<dyn Iterator<Item = Result<_, _>> + Send> = Box::new(it);
            let stream = stream::iter(iter_box).map(|res| res.map(|e| e.value().clone()).map_err(AppError::from));
            Ok(stream)
        }
    };


    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() {
            let storage = STORAGE.clone();
            let val = #column_type::default();
            let read_tx = storage.db.begin_read().expect("Failed to begin read transaction");
            let tx_context = #entity_name::begin_read_tx(&read_tx).expect("Failed to begin read transaction context");
            let pk_stream = #entity_name::#fn_name(tx_context, val).expect("Stream creation failed");
            let pks = pk_stream.try_collect::<Vec<#pk_type>>().await.expect("Failed to collect stream");
            assert_eq!(vec![#pk_type::default()], pks);
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let rt = Runtime::new().unwrap();
            let storage = STORAGE.clone();
            b.iter(|| {
                rt.block_on(async {
                    let read_tx = storage.db.begin_read().unwrap();
                    let tx_context = #entity_name::begin_read_tx(&read_tx).expect("Failed to begin read transaction context");
                    let pk_stream = #entity_name::#fn_name(tx_context, #column_type::default()).expect("Stream creation failed");
                    pk_stream.try_collect::<Vec<#pk_type>>().await.expect("Failed to collect stream");
                })
            });
        }
    });
    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            _entity_name: entity_name.clone(),
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![Path(vec![PathExpr {
                name: column_name.clone(),
                ty: column_type.clone(),
                description: "Secondary index column".to_string(),
                sample: quote! { #column_type::default().url_encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match state.storage.db.begin_read()
                        .map_err(AppError::from)
                        .and_then(|tx| #entity_name::begin_read_tx(&tx).map_err(AppError::from))
                        .and_then(|tx_context| #entity_name::#fn_name(tx_context, #column_name)) {
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
