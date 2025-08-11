use crate::rest::HttpParams::{FromBody, FromQuery};
use crate::rest::{BodyExpr, EndpointTag, FunctionDef, HttpMethod, QueryExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;

pub fn stream_range_by_index_def(entity_name: &Ident, entity_type: &Type, column_name: &Ident, column_type: &Type, pk_type: &Type, table: &Ident, range_query_ty: &Type, stream_query_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("stream_range_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(
            tx: ReadTransaction,
            from: #column_type,
            until: #column_type,
            query: Option<#stream_query_type>
        ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<#entity_type, AppError>> + Send + 'static>>, AppError> {
            let mm_table = tx.open_multimap_table(#table)?;
            let outer_iter = mm_table.range::<#column_type>(from..until)?;
            let outer_stream = futures::stream::iter(outer_iter).map_err(AppError::from);
            let pk_stream = outer_stream.map_ok(|(_key, value_iter)| {
                futures::stream::iter(value_iter).map(|res| {
                    res.map_err(AppError::from).map(|guard| guard.value().clone())
                })
            });
            Ok(
                pk_stream
                    .try_flatten()
                    .map(move |pk_res| {
                        match pk_res {
                            Ok(pk) => {
                                if let Some(ref stream_query) = query {
                                    match Self::compose_with_filter(&tx, &pk, stream_query) {
                                        Ok(Some(entity)) => Some(Ok(entity)),
                                        Ok(None) => None,
                                        Err(e) => Some(Err(e)),
                                    }
                                } else {
                                    Some(Self::compose(&tx, &pk)) // <- already Result<T, AppError>
                                }
                            }
                            Err(e) => Some(Err(e)),
                        }
                    })
                    .filter_map(std::future::ready) // remove None
                    .boxed()
            )
        }
    };

    let test_with_filter_fn_name = format_ident!("{}_with_filter", fn_name);
    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() {
            let storage = STORAGE.clone();
            let read_tx = storage.db.begin_read().expect("Failed to begin read transaction");
            let from_value = #column_type::default();
            let until_value = #column_type::default().next_value().next_value();
            let entity_stream = #entity_name::#fn_name(read_tx, from_value, until_value, None).expect("Failed to range entities by index");
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
            let expected_entities = #entity_type::sample_many(2);
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given stream_range by index");
        }
        #[tokio::test]
        async fn #test_with_filter_fn_name() {
            let storage = STORAGE.clone();
            let read_tx = storage.db.begin_read().expect("Failed to begin read transaction");
            let pk = #pk_type::default();
            let from_value = #column_type::default();
            let until_value = #column_type::default().next_value().next_value().next_value();
            let query = #stream_query_type::sample();
            let entity_stream = #entity_name::#fn_name(read_tx, from_value, until_value, Some(query.clone())).expect("Failed to range entities by index");
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
            let expected_entity = #entity_type::sample_with_query(&pk, 0, &query).expect("Failed to create sample entity with query");
            assert_eq!(entities.len(), 1, "Expected only one entity to be returned for the given range by with filter");
            assert_eq!(entities[0], expected_entity, "RangeBy result is not equal to sample because it is filtered, query: {:?}", query);
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let rt = Runtime::new().unwrap();
            let storage = STORAGE.clone();
            let query = #stream_query_type::sample();
            b.iter(|| {
                rt.block_on(async {
                    let from_value = #column_type::default();
                    let until_value = #column_type::default().next_value().next_value().next_value();
                    let read_tx = storage.db.begin_read().unwrap();
                    let entity_stream = #entity_name::#fn_name(read_tx, from_value, until_value, Some(query.clone())).expect("Failed to range entities by index");
                    entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
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
            params: vec![FromQuery(QueryExpr {
                ty: range_query_ty.clone(),
                extraction: quote! { extract::Query(query): extract::Query<#range_query_ty> },
                samples: quote! { vec![#range_query_ty::sample()] },
            }), FromBody(BodyExpr {
                ty: syn::parse_quote! { Option<#stream_query_type> },
                extraction: quote! { MaybeJson(body): MaybeJson<#stream_query_type> },
                samples: quote! { vec![#stream_query_type::sample()] },
                required: false,
            })],
            method: HttpMethod::POST,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match state.storage.db.begin_read()
                        .map_err(AppError::from)
                        .and_then(|tx| #entity_name::#fn_name(tx, query.from, query.until, body)) {
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
            endpoint: format!("/{}/{}", entity_name.to_string().to_lowercase(), column_name.clone()),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}
