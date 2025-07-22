use crate::rest::HttpParams::{FromBody, FromPath};
use crate::rest::{FunctionDef, HttpMethod, PathExpr, BodyExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;

pub fn by_dict_def(
    entity_name: &Ident,
    column_name: &Ident,
    column_type: &Type,
    pk_type: &Type,
    value_to_dict_pk: &Ident,
    dict_index_table: &Ident,
    stream_parent_query_type: &Type,
    parent_type: &Type,
    parent_ident: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("stream_{}s_by_{}", parent_ident.to_string().to_lowercase(), column_name);
    let fn_stream = quote! {
        pub fn #fn_name(tx: ReadTransaction, val: #column_type, query: Option<#stream_parent_query_type>) -> Result<Pin<Box<dyn futures::Stream<Item = Result<#parent_type, AppError>> + Send + 'static>>, AppError> {
            let val2birth = tx.open_table(#value_to_dict_pk)?;
            let birth_guard = val2birth.get(&val)?;
            let mut unique_parent_pointers = Vec::new();
            if let Some(g) = birth_guard {
                let birth_id = g.value().clone();
                let birth2pks = tx.open_multimap_table(#dict_index_table)?;
                let mm_value = birth2pks.get(&birth_id)?;
                for guard in mm_value {
                    let pk = guard?.value().clone();
                    unique_parent_pointers.push(pk.parent);
                }
            };
            unique_parent_pointers.dedup();
            let stream = futures::stream::unfold(
                (unique_parent_pointers.into_iter(), tx, query),
                |(mut iter, tx, query)| async move {
                    match iter.next() {
                        Some(parent_pointer) => {
                            if let Some(ref stream_query) = query {
                                match #parent_type::compose_with_filter(&tx, &parent_pointer, stream_query) {
                                    Ok(Some(entity)) => Some((Ok(entity), (iter, tx, query))),
                                    Ok(None) => None,
                                    Err(e) => Some((Err(e), (iter, tx, query))),
                                }
                            } else {
                                Some((#parent_type::compose(&tx, &parent_pointer), (iter, tx, query)))
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
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let entity_stream = #entity_name::#fn_name(read_tx, val, None).expect("Failed to get parent entities by dictionary index");
            let parent_entities = entity_stream.try_collect::<Vec<#parent_type>>().await.expect("Failed to collect parent entity stream");
            let expected_entities = vec![#parent_type::sample()];
            assert_eq!(expected_entities, parent_entities, "Expected parent entities to be returned for the given dictionary index");
        }
        #[tokio::test]
        async fn #test_with_filter_fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let pk = #pk_type::default();
            let parent_pk = pk.parent();
            let query = #stream_parent_query_type::sample();
            let entity_stream = #entity_name::#fn_name(read_tx, val, Some(query.clone())).expect("Failed to get parent entities by index");
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
            let rt = Runtime::new().unwrap();
            let db = DB.clone();
            let query = #stream_parent_query_type::sample();
            b.iter(|| {
                rt.block_on(async {
                    let read_tx = db.begin_read().unwrap();
                    let parent_entity_stream = #entity_name::#fn_name(read_tx, #column_type::default(), Some(query.clone())).expect("Failed to get parent entities by index");
                    parent_entity_stream.try_collect::<Vec<#parent_type>>().await.expect("Failed to collect parent entity stream");
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
                description: "Secondary index column with dictionary".to_string(),
                sample: quote! { #column_type::default().encode() },
            }]), FromBody(BodyExpr {
                ty: syn::parse_quote! { #stream_parent_query_type },
                extraction: quote! { MaybeJson(body): MaybeJson<#stream_parent_query_type> },
                samples: quote! { vec![Some(#stream_parent_query_type::sample()), None ] },
            })],
            method: HttpMethod::POST,
            handler_name: format_ident!("{}", handler_fn_name),
            client_calls: vec![],
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match state.db.begin_read()
                        .map_err(AppError::from)
                        .and_then(|tx| #entity_name::#fn_name(tx, #column_name, body)) {
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
    table: &Ident,
    stream_parent_query_type: &Type,
    parent_type: &Type,
    parent_ident: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("stream_{}s_by_{}", parent_ident.to_string().to_lowercase(), column_name);
    let fn_stream = quote! {
        pub fn #fn_name(tx: ReadTransaction, val: #column_type, query: Option<#stream_parent_query_type>) -> Result<Pin<Box<dyn futures::Stream<Item = Result<#parent_type, AppError>> + Send + 'static>>, AppError> {
            let mm_table = tx.open_multimap_table(#table).map_err(AppError::from)?;
            let iter = mm_table.get(&val).map_err(AppError::from)?;
            let mut unique_parent_pointers = Vec::new();
            for guard in iter {
                let pk = guard?.value().clone();
                unique_parent_pointers.push(pk.parent);
            }
            unique_parent_pointers.dedup();
            let stream = futures::stream::unfold(
                (unique_parent_pointers.into_iter(), tx, query),
                |(mut iter, tx, query)| async move {
                    match iter.next() {
                        Some(parent_pointer) => {
                            if let Some(ref stream_query) = query {
                                match #parent_type::compose_with_filter(&tx, &parent_pointer, stream_query) {
                                    Ok(Some(entity)) => Some((Ok(entity), (iter, tx, query))),
                                    Ok(None) => None,
                                    Err(e) => Some((Err(e), (iter, tx, query))),
                                }
                            } else {
                                Some((#parent_type::compose(&tx, &parent_pointer), (iter, tx, query)))
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
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let entity_stream = #entity_name::#fn_name(read_tx, val, None).expect("Failed to get parent entities by index");
            let parent_entities = entity_stream.try_collect::<Vec<#parent_type>>().await.expect("Failed to collect parent entity stream");
            let expected_entities = vec![#parent_type::sample()];
            assert_eq!(expected_entities, parent_entities, "Expected parent entities to be returned for the given index");
        }
        #[tokio::test]
        async fn #test_with_filter_fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let pk = #pk_type::default();
            let parent_pk = pk.parent();
            let query = #stream_parent_query_type::sample();
            let entity_stream = #entity_name::#fn_name(read_tx, val, Some(query.clone())).expect("Failed to get parent entities by index");
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
            let rt = Runtime::new().unwrap();
            let db = DB.clone();
            let query = #stream_parent_query_type::sample();
            b.iter(|| {
                rt.block_on(async {
                    let read_tx = db.begin_read().unwrap();
                    let parent_entity_stream = #entity_name::#fn_name(read_tx, #column_type::default(), Some(query.clone())).expect("Failed to get parent entities by index");
                    parent_entity_stream.try_collect::<Vec<#parent_type>>().await.expect("Failed to collect parent entity stream");
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
            params: vec![
                FromPath(vec![PathExpr {
                    name: column_name.clone(),
                    ty: column_type.clone(),
                    description: "Secondary index column".to_string(),
                    sample: quote! { #column_type::default().encode() },
                }]
                ), FromBody(BodyExpr {
                    ty: syn::parse_quote! { #stream_parent_query_type },
                    extraction: quote! { MaybeJson(body): MaybeJson<#stream_parent_query_type> },
                    samples: quote! { vec![Some(#stream_parent_query_type::sample()), None ] },
                })
            ],
            method: HttpMethod::POST,
            handler_name: format_ident!("{}", handler_fn_name),
            client_calls: vec![],
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match state.db.begin_read()
                        .map_err(AppError::from)
                        .and_then(|tx| #entity_name::#fn_name(tx, #column_name, body)) {
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
