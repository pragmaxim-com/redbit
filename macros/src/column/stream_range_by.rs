use crate::endpoint::EndpointDef;
use crate::field_parser::EntityDef;
use crate::rest::HttpParams::{Body, Query};
use crate::rest::{BodyExpr, EndpointTag, FunctionDef, HttpMethod, QueryExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn stream_range_by_index_def(entity_def: &EntityDef, column_name: &Ident, column_type: &Type, index_table: &Ident, range_query_ty: &Type) -> FunctionDef {
    let EntityDef { key_def, entity_name, entity_type, query_type, read_ctx_type, ..} = &entity_def;
    let pk_type = &key_def.field_def().tpe;
    let fn_name = format_ident!("stream_range_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(
            tx_context: #read_ctx_type,
            from: #column_type,
            until: #column_type,
            query: Option<#query_type>,
        ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<#entity_type, AppError>> + Send>>, AppError> {
            let outer_iter = tx_context.#index_table.range_keys::<#column_type>(from..until)?;
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
                                    match Self::compose_with_filter(&tx_context, pk, stream_query) {
                                        Ok(Some(entity)) => Some(Ok(entity)),
                                        Ok(None) => None,
                                        Err(e) => Some(Err(e)),
                                    }
                                } else {
                                    Some(Self::compose(&tx_context, pk)) // <- already Result<T, AppError>
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
            let (storage_owner, storage) = &*STORAGE;
            let from_value = #column_type::default();
            let until_value = #column_type::default().nth_value(2);
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entity_stream = #entity_name::#fn_name(tx_context, from_value, until_value, None).expect("Failed to range entities by index");
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
            let expected_entities = #entity_type::sample_many(2);
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given stream_range by index");
        }
        #[tokio::test]
        async fn #test_with_filter_fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let pk = #pk_type::default();
            let from_value = #column_type::default();
            let until_value = #column_type::default().nth_value(3);
            let query = #query_type::sample();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entity_stream = #entity_name::#fn_name(tx_context, from_value, until_value, Some(query.clone())).expect("Failed to range entities by index");
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
            let expected_entity = #entity_type::sample_with_query(pk, 0, &query).expect("Failed to create sample entity with query");
            assert_eq!(entities.len(), 1, "Expected only one entity to be returned for the given range by with filter");
            assert_eq!(entities[0], expected_entity, "RangeBy result is not equal to sample because it is filtered, query: {:?}", query);
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
                    let from_value = #column_type::default();
                    let until_value = #column_type::default().nth_value(3);
                    let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
                    let entity_stream = #entity_name::#fn_name(tx_context, from_value, until_value, Some(query.clone())).expect("Failed to range entities by index");
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
            params: vec![Query(QueryExpr {
                ty: range_query_ty.clone(),
                extraction: quote! { extract::Query(query): extract::Query<#range_query_ty> },
                samples: quote! { vec![#range_query_ty::sample()] },
            }), Body(BodyExpr {
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
                        .and_then(|tx_context| #entity_name::#fn_name(tx_context, query.from, query.until, body)) {
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
