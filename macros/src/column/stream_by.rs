use crate::endpoint::EndpointDef;
use crate::field_parser::EntityDef;
use crate::rest::HttpParams::{Body, Path};
use crate::rest::{BodyExpr, EndpointTag, FunctionDef, HttpMethod, PathExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn by_dict_def(
    entity_def: &EntityDef,
    column_name: &Ident,
    column_type: &Type,
    dict_table_var: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("stream_by_{}", column_name);
    let EntityDef { key_def, entity_name, entity_type, query_type, read_ctx_type, ..} = &entity_def;
    let pk_type = key_def.field_def().tpe;
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: #read_ctx_type, val: #column_type, query: Option<#query_type>) -> Result<impl futures::Stream<Item = Result<#entity_type, AppError>> + Send, AppError> {
            let iter = tx_context.#dict_table_var.dict_keys(val)?.into_iter().flatten().map(|res| res.map(|kg| kg.value()));
            Self::compose_many_stream(tx_context, iter, query)
        }
    };

    let test_with_filter_fn_name = format_ident!("{}_with_filter", fn_name);
    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() -> Result<(), AppError> {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage)?;
            let entity_stream = #entity_name::#fn_name(tx_context, val, None)?;
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await?;
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given dictionary index");
            Ok(())
        }
        #[tokio::test]
        async fn #test_with_filter_fn_name() -> Result<(), AppError> {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let pk = #pk_type::default();
            let query = #query_type::sample();
            let tx_context = #entity_name::begin_read_ctx(&storage)?;
            let entity_stream = #entity_name::#fn_name(tx_context, val, Some(query.clone()))?;
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await?;
            let expected_entity = #entity_type::sample_with_query(pk, &query).expect("Failed to create sample entity");
            assert_eq!(entities.len(), 1, "Expected only one entity to be returned for the given dictionary index with filter");
            assert_eq!(entities[0], expected_entity, "Dict result is not equal to sample because it is filtered, query: {:?}", query);
            Ok(())
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let query = #query_type::sample();
            let rt = Runtime::new().unwrap();
            b.iter(|| {
                rt.block_on(async {
                    let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
                    let val = #column_type::default();
                    let entity_stream = #entity_name::#fn_name(tx_context, val, Some(query.clone())).expect("Failed to get entities by index");
                    entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
                })
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: Some(entity_type.clone()),
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![Path(vec![PathExpr {
                name: column_name.clone(),
                ty: column_type.clone(),
                description: "Secondary index column with dictionary".to_string(),
                sample: quote! { #column_type::default().url_encode() },
            }]), Body(BodyExpr {
                ty: syn::parse_quote! { Option<#query_type> },
                extraction: quote! { MaybeJson(body): MaybeJson<#query_type> },
                samples: quote! { vec![#query_type::sample()] },
                required: false,
            })],
            method: HttpMethod::POST,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match #entity_name::begin_read_ctx(&state.storage)
                        .and_then(|tx_context| #entity_name::#fn_name(tx_context, #column_name, body)) {
                            Ok(stream) => axum_streams::StreamBodyAs::json_nl_with_errors(stream).header("Content-Type", HeaderValue::from_str("application/x-ndjson").unwrap()).into_response(),
                            Err(err)   => err.into_response(),
                    }
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/x-ndjson", body = #entity_type),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), column_name, column_name),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}

pub fn by_index_def(entity_def: &EntityDef, column_name: &Ident, column_type: &Type, index_table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("stream_by_{}", column_name);
    let EntityDef { key_def, entity_name, entity_type, query_type, read_ctx_type, ..} = &entity_def;
    let pk_type = key_def.field_def().tpe;
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: #read_ctx_type, val: #column_type, query: Option<#query_type>) -> Result<impl futures::Stream<Item = Result<#entity_type, AppError>> + Send, AppError> {
            let iter = tx_context.#index_table.index_keys(val)?.map(|res| res.map(|kg| kg.value()));
            Self::compose_many_stream(tx_context, iter, query)
        }
    };

    let test_with_filter_fn_name = format_ident!("{}_with_filter", fn_name);
    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() -> Result<(), AppError> {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage)?;
            let entity_stream = #entity_name::#fn_name(tx_context, val, None)?;
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await?;
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given index");
            Ok(())
        }
        #[tokio::test]
        async fn #test_with_filter_fn_name() -> Result<(), AppError> {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let pk = #pk_type::default();
            let query = #query_type::sample();
            let tx_context = #entity_name::begin_read_ctx(&storage)?;
            let entity_stream = #entity_name::#fn_name(tx_context, val, Some(query.clone()))?;
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await?;
            let expected_entity = #entity_type::sample_with_query(pk, &query).expect("Failed to create sample entity");
            assert_eq!(entities.len(), 1, "Expected only one entity to be returned");
            assert_eq!(entities[0], expected_entity, "Indexed result is not equal to sample, query: {:?}", query);
            Ok(())
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let query = #query_type::sample();
            let rt = Runtime::new().unwrap();
            b.iter(|| {
                rt.block_on(async {
                    let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
                    let entity_stream = #entity_name::#fn_name(tx_context, #column_type::default(), Some(query.clone())).expect("Failed to get entities by index");
                    entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
                })
            });
        }
    });
    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: Some(entity_type.clone()),
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![
                Path(vec![PathExpr {
                    name: column_name.clone(),
                    ty: column_type.clone(),
                    description: "Secondary index column".to_string(),
                    sample: quote! { #column_type::default().url_encode() },
                }]
                ), Body(BodyExpr {
                    ty: syn::parse_quote! { Option<#query_type> },
                    extraction: quote! { MaybeJson(body): MaybeJson<#query_type> },
                    samples: quote! { vec![#query_type::sample()] },
                    required: false,
                })
            ],
            method: HttpMethod::POST,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match #entity_name::begin_read_ctx(&state.storage)
                        .and_then(|tx_context| #entity_name::#fn_name(tx_context, #column_name, body)) {
                            Ok(stream) => axum_streams::StreamBodyAs::json_nl_with_errors(stream).header("Content-Type", HeaderValue::from_str("application/x-ndjson").unwrap()).into_response(),
                            Err(err)   => err.into_response(),
                    }
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/x-ndjson", body = #entity_type),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), column_name, column_name),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}
