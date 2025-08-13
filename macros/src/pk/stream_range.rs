use crate::rest::HttpParams::{Body, Query};
use crate::rest::{BodyExpr, EndpointTag, FunctionDef, HttpMethod, QueryExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;
use crate::field_parser::FieldDef;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_field_def: &FieldDef, table: &Ident, range_query_ty: &Type, stream_query_type: &Type, no_columns: bool) -> FunctionDef {
    let pk_name = pk_field_def.name.clone();
    let pk_type = pk_field_def.tpe.clone();

    let fn_name = format_ident!("stream_range");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx: StorageReadTx, from: #pk_type, until: #pk_type, query: Option<#stream_query_type>) -> Result<Pin<Box<dyn futures::Stream<Item = Result<#entity_type, AppError>> + Send + 'static>>, AppError> {
                let table_pk_9 = tx.open_table(#table)?;
                let range = from..until;
                let iter_box = Box::new(table_pk_9.range::<#pk_type>(range)?);
                let stream = futures::stream::unfold(
                    (iter_box, tx, query),
                    |(mut iter, tx, query)| async move {
                        match iter.next() {
                            Some(Ok((key, _val))) => {
                                let pk = key.value().clone();
                                if let Some(ref stream_query) = query {
                                    match Self::compose_with_filter(&tx, &pk, stream_query) {
                                        Ok(Some(entity)) => Some((Ok(entity), (iter, tx, query))),
                                        Ok(None) => None,
                                        Err(e) => Some((Err(e), (iter, tx, query))),
                                    }
                                } else {
                                    Some((Self::compose(&tx, &pk), (iter, tx, query)))
                                }
                            }
                            Some(Err(e)) => Some((Err(AppError::from(e)), (iter, tx, query))),
                            None => None,
                        }
                    },
                ).boxed();
                Ok(stream)
            }
        };

    let test_with_filter_fn_name = format_ident!("{}_with_filter", fn_name);
    let test_stream_with_filter = if no_columns {
        None
    } else {
        Some(quote! {
            #[tokio::test]
            async fn #test_with_filter_fn_name() {
                let storage = STORAGE.clone();
                let read_tx = storage.begin_read().expect("Failed to begin read transaction");
                let pk = #pk_type::default();
                let from_value = #pk_type::default();
                let until_value = #pk_type::default().next_index().next_index().next_index();
                let query = #stream_query_type::sample();
                let entity_stream = #entity_name::#fn_name(read_tx, from_value, until_value, Some(query.clone())).expect("Failed to range entities by pk");
                let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
                let expected_entity = #entity_type::sample_with_query(&pk, 0, &query).expect("Failed to create sample entity with query");
                assert_eq!(entities.len(), 1, "Expected only one entity to be returned for the given stream range with filter");
                assert_eq!(entities[0], expected_entity, "Stream Range result is not equal to sample because it is filtered, query: {:?}", query);
            }
        })
    };

    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() {
            let storage = STORAGE.clone();
            let read_tx = storage.begin_read().expect("Failed to begin read transaction");
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next_index().next_index();
            let entity_stream = #entity_name::#fn_name(read_tx, from_value, until_value, None).expect("Failed to range entities by pk");
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
            let expected_entities = #entity_type::sample_many(2);
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given range");
        }
        #test_stream_with_filter
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
                    let from_value = #pk_type::default();
                    let until_value = #pk_type::default().next_index().next_index().next_index();
                    let read_tx = storage.begin_read().unwrap();
                    let entity_stream = #entity_name::#fn_name(read_tx, from_value, until_value, Some(query.clone())).expect("Failed to range entities by pk");
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
            params: vec![Query(QueryExpr {
                ty: range_query_ty.clone(),
                extraction: quote! { extract::Query(query): extract::Query<#range_query_ty> },
                samples: quote! { vec![#range_query_ty::sample()] },
            }), Body(BodyExpr {
                ty: syn::parse_quote! { Option<#stream_query_type> },
                extraction: quote! { MaybeJson(body): MaybeJson<#stream_query_type> },
                samples: quote! { vec![#stream_query_type::sample()] },
                required: false,
            })],
            method: HttpMethod::POST,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match state.storage.begin_read()
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
            endpoint: format!("/{}/{}", entity_name.to_string().to_lowercase(), pk_name.clone()),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}