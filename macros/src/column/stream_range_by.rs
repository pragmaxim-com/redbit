use crate::rest::HttpParams::{FromBody, FromQuery};
use crate::rest::{BodyExpr, FunctionDef, HttpMethod, QueryExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;

pub fn stream_range_by_index_def(entity_name: &Ident, entity_type: &Type, column_name: &Ident, column_type: &Type, table: &Ident, range_query: Type, stream_query_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("stream_range_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(
            tx: ReadTransaction,
            from: #column_type,
            until: #column_type,
            query: Option<#stream_query_type>
        ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<#entity_type, AppError>> + Send + 'static>>, AppError> {
            if from >= until {
                return Err(AppError::BadRequest("Range cannot be empty".to_string()));
            }

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
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let from_value = #column_type::default();
            let until_value = #column_type::default().next().next();
            let entity_stream = #entity_name::#fn_name(read_tx, from_value, until_value, None).expect("Failed to range entities by index");
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
            let expected_entities = #entity_type::sample_many(2);
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given stream_range by index");
        }
        #[tokio::test]
        async fn #test_with_filter_fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let from_value = #column_type::default();
            let until_value = #column_type::default().next().next().next();
            let query = #stream_query_type::sample();
            let entity_stream = #entity_name::#fn_name(read_tx, from_value, until_value, Some(query.clone())).expect("Failed to range entities by index");
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(entities, expected_entities, "Only the default valued entity, filter is set for default values, query: {:?}", query);
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let rt = Runtime::new().unwrap();
            let db = DB.clone();
            let query = #stream_query_type::sample();
            b.iter(|| {
                rt.block_on(async {
                    let from_value = #column_type::default();
                    let until_value = #column_type::default().next().next().next();
                    let read_tx = db.begin_read().unwrap();
                    let entity_stream = #entity_name::#fn_name(read_tx, from_value, until_value, Some(query.clone())).expect("Failed to range entities by index");
                    entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
                })
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            params: vec![FromQuery(QueryExpr {
                ty: range_query.clone(),
                extraction: quote! { extract::Query(query): extract::Query<#range_query> },
                samples: quote! { vec![#range_query::sample()] },
            }), FromBody(BodyExpr {
                ty: syn::parse_quote! { #stream_query_type },
                extraction: quote! { MaybeJson(body): MaybeJson<#stream_query_type> },
                samples: quote! { vec![Some(#stream_query_type::sample()), None] },
            })],
            method: HttpMethod::POST,
            handler_name: format_ident!("{}", handler_fn_name),
            client_call: None,
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match state.db.begin_read()
                        .map_err(AppError::from)
                        .and_then(|tx| #entity_name::#fn_name(tx, query.from, query.until, body)) {
                            Ok(stream) => axum_streams::StreamBodyAs::json_nl_with_errors(stream).into_response(),
                            Err(err)   => err.into_response(),
                    }
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/json", body = #entity_type),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{}", entity_name.to_string().to_lowercase(), column_name.clone()),
        }),
        test_stream,
        bench_stream
    }
}
