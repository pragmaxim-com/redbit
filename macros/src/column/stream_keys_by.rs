use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::Path;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod, PathExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::field_parser::EntityDef;

/// Generates a streaming SSE endpoint definition for querying primary keys by a dictionary index.
pub fn by_dict_def(entity_def: &EntityDef, column_name: &Ident, column_type: &Type, dict_table_var: &Ident) -> FunctionDef {
    let key_def = &entity_def.key_def.field_def();
    let pk_name = &key_def.name;
    let pk_type = &key_def.tpe;
    let fn_name = format_ident!("stream_{}s_by_{}", pk_name, column_name);
    let entity_name = &entity_def.entity_name;
    let read_ctx_type = &entity_def.read_ctx_type;
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: #read_ctx_type, val: #column_type) -> Result<impl futures::Stream<Item = Result<#pk_type, AppError>> + Send, AppError> {
            let iter = tx_context.#dict_table_var.dict_keys(val)?.into_iter().flatten().map(|res| res.map(|g| g.value().clone()).map_err(AppError::from));
            Ok(stream::iter(iter))
        }
    };

    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let pk_stream = #entity_name::#fn_name(tx_context, val).expect("Stream creation failed");
            let pks = pk_stream.try_collect::<Vec<#pk_type>>().await.expect("Failed to collect stream");
            assert_eq!(vec![#pk_type::default()], pks);
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let rt = Runtime::new().unwrap();
            b.iter(|| {
                rt.block_on(async {
                    let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
                    let val = #column_type::default();
                    let pk_stream = #entity_name::#fn_name(tx_context, val).expect("Stream creation failed");
                    pk_stream.try_collect::<Vec<#pk_type>>().await.expect("Failed to collect stream");
                })
            });
        }
    });
    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: Some(pk_type.clone()),
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
                   match #entity_name::begin_read_ctx(&state.storage)
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
pub fn by_index_def(entity_def: &EntityDef, column_name: &Ident, column_type: &Type, index_table: &Ident) -> FunctionDef {
    let entity_name = &entity_def.entity_name;
    let read_ctx_type = &entity_def.read_ctx_type;
    let key_def = &entity_def.key_def.field_def();
    let pk_name = &key_def.name;
    let pk_type = &key_def.tpe;
    let fn_name = format_ident!("stream_{}s_by_{}", pk_name, column_name);
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: #read_ctx_type, val: #column_type) -> Result<impl futures::Stream<Item = Result<#pk_type, AppError>> + Send, AppError> {
            Ok(stream::iter(tx_context.#index_table.index_keys(&val)?).map(|res| res.map(|e| e.value().clone()).map_err(AppError::from)))
        }
    };


    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let pk_stream = #entity_name::#fn_name(tx_context, val).expect("Stream creation failed");
            let pks = pk_stream.try_collect::<Vec<#pk_type>>().await.expect("Failed to collect stream");
            assert_eq!(vec![#pk_type::default()], pks);
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let rt = Runtime::new().unwrap();
            b.iter(|| {
                rt.block_on(async {
                    let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
                    let val = #column_type::default();
                    let pk_stream = #entity_name::#fn_name(tx_context, val).expect("Stream creation failed");
                    pk_stream.try_collect::<Vec<#pk_type>>().await.expect("Failed to collect stream");
                })
            });
        }
    });
    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: Some(pk_type.clone()),
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
                   match #entity_name::begin_read_ctx(&state.storage)
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
