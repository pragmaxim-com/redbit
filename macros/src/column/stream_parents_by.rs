use crate::endpoint::EndpointDef;
use crate::field_parser::OneToManyParentDef;
use crate::rest::HttpParams::{Body, Path};
use crate::rest::{BodyExpr, EndpointTag, FunctionDef, HttpMethod, PathExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn by_dict_def(
    entity_name: &Ident,
    column_name: &Ident,
    column_type: &Type,
    pk_type: &Type,
    dict_table_var: &Ident,
    parent_def: &OneToManyParentDef,
) -> FunctionDef {
    let parent_ident = &parent_def.parent_ident;
    let parent_type = &parent_def.parent_type;
    let stream_parent_query_type = &parent_def.stream_query_ty;
    let parent_tx_context_type = &parent_def.tx_context_ty;
    let parent_one2many_field_name = format_ident!("{}s", entity_name.to_string().to_lowercase());

    let fn_name = format_ident!("stream_{}s_by_{}", parent_ident.to_string().to_lowercase(), column_name);
    let fn_stream = quote! {
        pub fn #fn_name(parent_tx_context: #parent_tx_context_type, val: #column_type, query: Option<#stream_parent_query_type>) -> Result<Pin<Box<dyn futures::Stream<Item = Result<#parent_type, AppError>> + Send>>, AppError> {
            let multi_value_opt = parent_tx_context.#parent_one2many_field_name.#dict_table_var.get_keys(val)?;
            let mut unique_parent_pointers =
                match multi_value_opt {
                    None => Vec::new(),
                    Some(multi_value) => {
                        let mut pointers = Vec::new();
                        for pk_guard in multi_value {
                            let pk = pk_guard?.value();
                            pointers.push(pk.parent);
                        }
                        pointers
                    }
                };
            unique_parent_pointers.dedup();
            let stream = futures::stream::unfold(
                (unique_parent_pointers.into_iter(), parent_tx_context, query),
                |(mut iter, parent_tx_context, query)| async move {
                    match iter.next() {
                        Some(parent_pointer) => {
                            if let Some(ref stream_query) = query {
                                match #parent_type::compose_with_filter(&parent_tx_context, &parent_pointer, stream_query) {
                                    Ok(Some(entity)) => Some((Ok(entity), (iter, parent_tx_context, query))),
                                    Ok(None) => None,
                                    Err(e) => Some((Err(e), (iter, parent_tx_context, query))),
                                }
                            } else {
                                Some((#parent_type::compose(&parent_tx_context, &parent_pointer), (iter, parent_tx_context, query)))
                            }
                        }
                        None => None,
                    }
                },
            ).boxed();
            Ok(stream)
        }
    };


    let test_with_filter_fn_name = format_ident!("{}_with_filter", fn_name);
    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let tx_context = #parent_type::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entity_stream = #entity_name::#fn_name(tx_context, val, None).expect("Failed to get parent entities by dictionary index");
            let parent_entities = entity_stream.try_collect::<Vec<#parent_type>>().await.expect("Failed to collect parent entity stream");
            let expected_entities = vec![#parent_type::sample()];
            assert_eq!(expected_entities, parent_entities, "Expected parent entities to be returned for the given dictionary index");
        }
        #[tokio::test]
        async fn #test_with_filter_fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let pk = #pk_type::default();
            let parent_pk = pk.parent();
            let query = #stream_parent_query_type::sample();
            let tx_context = #parent_type::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entity_stream = #entity_name::#fn_name(tx_context, val, Some(query.clone())).expect("Failed to get parent entities by index");
            let parent_entities = entity_stream.try_collect::<Vec<#parent_type>>().await.expect("Failed to collect parent entity stream");
            let expected_entity = #parent_type::sample_with_query(&parent_pk, 0, &query).expect("Failed to create sample entity with query");
            assert_eq!(parent_entities.len(), 1, "Expected only one parent entity to be returned for the given dictionary index with filter");
            assert_eq!(parent_entities[0], expected_entity, "Dict result is not equal to sample because it is filtered, query: {:?}", query);
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let query = #stream_parent_query_type::sample();
            let rt = Runtime::new().unwrap();
            b.iter(|| {
                rt.block_on(async {
                    let tx_context = #parent_type::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
                    let val = #column_type::default();
                    let parent_entity_stream = #entity_name::#fn_name(tx_context, val, Some(query.clone())).expect("Failed to get parent entities by index");
                    parent_entity_stream.try_collect::<Vec<#parent_type>>().await.expect("Failed to collect parent entity stream");
                })
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: Some(parent_type.clone()),
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![Path(vec![PathExpr {
                name: column_name.clone(),
                ty: column_type.clone(),
                description: "Secondary index column with dictionary".to_string(),
                sample: quote! { #column_type::default().url_encode() },
            }]), Body(BodyExpr {
                ty: syn::parse_quote! { Option<#stream_parent_query_type> },
                extraction: quote! { MaybeJson(body): MaybeJson<#stream_parent_query_type> },
                samples: quote! { vec![#stream_parent_query_type::sample()] },
                required: false,
            })],
            method: HttpMethod::POST,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match #parent_type::begin_read_ctx(&state.storage)
                        .and_then(|tx_context| #entity_name::#fn_name(tx_context, #column_name, body)) {
                            Ok(stream) => axum_streams::StreamBodyAs::json_nl_with_errors(stream).header("Content-Type", HeaderValue::from_str("application/x-ndjson").unwrap()).into_response(),
                            Err(err)   => err.into_response(),
                    }
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/x-ndjson", body = #parent_type),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{}/{{{}}}/{}", entity_name.to_string().to_lowercase(), column_name, column_name, parent_ident.to_string().to_lowercase()),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}

pub fn by_index_def(
    entity_name: &Ident,
    column_name: &Ident,
    column_type: &Type,
    pk_type: &Type,
    index_table: &Ident,
    parent_def: &OneToManyParentDef,
) -> FunctionDef {
    let parent_ident = &parent_def.parent_ident;
    let parent_type = &parent_def.parent_type;
    let stream_parent_query_type = &parent_def.stream_query_ty;
    let parent_tx_context_type = &parent_def.tx_context_ty;
    let parent_one2many_field_name = format_ident!("{}s", entity_name.to_string().to_lowercase());

    let fn_name = format_ident!("stream_{}s_by_{}", parent_ident.to_string().to_lowercase(), column_name);
    let fn_stream = quote! {
        pub fn #fn_name(parent_tx_context: #parent_tx_context_type, val: #column_type, query: Option<#stream_parent_query_type>) -> Result<Pin<Box<dyn futures::Stream<Item = Result<#parent_type, AppError>> + Send>>, AppError> {
            let iter = parent_tx_context.#parent_one2many_field_name.#index_table.get_keys(&val)?;
            let mut unique_parent_pointers = Vec::new();
            for guard in iter {
                let pk = guard?.value().clone();
                unique_parent_pointers.push(pk.parent);
            }
            unique_parent_pointers.dedup();
            let stream = futures::stream::unfold(
                (unique_parent_pointers.into_iter(), parent_tx_context, query),
                |(mut iter, parent_tx_context, query)| async move {
                    match iter.next() {
                        Some(parent_pointer) => {
                            if let Some(ref stream_query) = query {
                                match #parent_type::compose_with_filter(&parent_tx_context, &parent_pointer, stream_query) {
                                    Ok(Some(entity)) => Some((Ok(entity), (iter, parent_tx_context, query))),
                                    Ok(None) => None,
                                    Err(e) => Some((Err(e), (iter, parent_tx_context, query))),
                                }
                            } else {
                                Some((#parent_type::compose(&parent_tx_context, &parent_pointer), (iter, parent_tx_context, query)))
                            }
                        }
                        None => None,
                    }
                },
            ).boxed();
            Ok(stream)
        }
    };

    let test_with_filter_fn_name = format_ident!("{}_with_filter", fn_name);
    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let tx_context = #parent_type::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entity_stream = #entity_name::#fn_name(tx_context, val, None).expect("Failed to get parent entities by index");
            let parent_entities = entity_stream.try_collect::<Vec<#parent_type>>().await.expect("Failed to collect parent entity stream");
            let expected_entities = vec![#parent_type::sample()];
            assert_eq!(expected_entities, parent_entities, "Expected parent entities to be returned for the given index");
        }
        #[tokio::test]
        async fn #test_with_filter_fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let pk = #pk_type::default();
            let parent_pk = pk.parent();
            let query = #stream_parent_query_type::sample();
            let tx_context = #parent_type::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entity_stream = #entity_name::#fn_name(tx_context, val, Some(query.clone())).expect("Failed to get parent entities by index");
            let parent_entities = entity_stream.try_collect::<Vec<#parent_type>>().await.expect("Failed to collect parent entity stream");
            let expected_entity = #parent_type::sample_with_query(&parent_pk, 0, &query).expect("Failed to create sample entity with query");
            assert_eq!(parent_entities.len(), 1, "Expected only one parent entity to be returned for the given index with filter");
            assert_eq!(parent_entities[0], expected_entity, "Indexed result is not equal to sample because it is filtered, query: {:?}", query);
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let query = #stream_parent_query_type::sample();
            let rt = Runtime::new().unwrap();
            b.iter(|| {
                rt.block_on(async {
                    let tx_context = #parent_type::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
                    let val = #column_type::default();
                    let parent_entity_stream = #entity_name::#fn_name(tx_context, val, Some(query.clone())).expect("Failed to get parent entities by index");
                    parent_entity_stream.try_collect::<Vec<#parent_type>>().await.expect("Failed to collect parent entity stream");
                })
            });
        }
    });
    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: Some(parent_type.clone()),
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
                    ty: syn::parse_quote! { Option<#stream_parent_query_type> },
                    extraction: quote! { MaybeJson(body): MaybeJson<#stream_parent_query_type> },
                    samples: quote! { vec![#stream_parent_query_type::sample()] },
                    required: false,
                })
            ],
            method: HttpMethod::POST,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match #parent_type::begin_read_ctx(&state.storage)
                        .and_then(|tx_context| #entity_name::#fn_name(tx_context, #column_name, body)) {
                            Ok(stream) => axum_streams::StreamBodyAs::json_nl_with_errors(stream).header("Content-Type", HeaderValue::from_str("application/x-ndjson").unwrap()).into_response(),
                            Err(err)   => err.into_response(),
                    }
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/x-ndjson", body = #parent_type),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{}/{{{}}}/{}", entity_name.to_string().to_lowercase(), column_name, column_name, parent_ident.to_string().to_lowercase()),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}
