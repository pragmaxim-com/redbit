use crate::endpoint::EndpointDef;
use crate::field_parser::EntityDef;
use crate::rest::HttpParams::{Body, Query};
use crate::rest::{BodyExpr, EndpointTag, FunctionDef, HttpMethod, QueryExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_def: &EntityDef, table: &Ident, range_query_ty: &Type, no_columns: bool) -> FunctionDef {
    let EntityDef { key_def, entity_name, entity_type, query_type, read_ctx_type, ..} = &entity_def;
    let key_def = key_def.field_def();
    let pk_name = &key_def.name;
    let pk_type = &key_def.tpe;
    let fn_name = format_ident!("stream_range");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx_context: #read_ctx_type, from: #pk_type, until: #pk_type, query: Option<#query_type>) -> Result<impl futures::Stream<Item = Result<#entity_type, AppError>> + Send, AppError> {
                let range = from..until;
                let iter = tx_context.#table.range::<#pk_type>(range)?.map(|res| res.map(|(kg, _)| kg.value()));
                Self::compose_many_stream(tx_context, iter, query)
            }
        };

    let test_with_filter_fn_name = format_ident!("{}_with_filter", fn_name);
    let test_stream_with_filter = if no_columns {
        None
    } else {
        Some(quote! {
            #[tokio::test]
            async fn #test_with_filter_fn_name() -> Result<(), AppError> {
                let (storage_owner, storage) = &*STORAGE;
                let pk = #pk_type::default();
                let from_value = #pk_type::default();
                let until_value = #pk_type::default().next_index().next_index().next_index();
                let query = #query_type::sample();
                let tx_context = #entity_name::begin_read_ctx(&storage)?;
                let entity_stream = #entity_name::#fn_name(tx_context, from_value, until_value, Some(query.clone()))?;
                let entities = entity_stream.try_collect::<Vec<#entity_type>>().await?;
                let expected_entity = #entity_type::sample_with_query(pk, &query).expect("Failed to create sample entity");
                assert_eq!(entities.len(), 1, "Expected only one entity to be returned for the given stream range with filter");
                assert_eq!(entities[0], expected_entity, "Stream Range result is not equal to sample because it is filtered, query: {:?}", query);
                Ok(())
            }
        })
    };

    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() -> Result<(), AppError> {
            let (storage_owner, storage) = &*STORAGE;
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next_index().next_index();
            let tx_context = #entity_name::begin_read_ctx(&storage)?;
            let entity_stream = #entity_name::#fn_name(tx_context, from_value, until_value, None)?;
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await?;
            let expected_entities = #entity_type::sample_many(Default::default(), 2);
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given range");
            Ok(())
        }
        #test_stream_with_filter
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
                    let from_value = #pk_type::default();
                    let until_value = #pk_type::default().next_index().next_index().next_index();
                    let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
                    let entity_stream = #entity_name::#fn_name(tx_context, from_value, until_value, Some(query.clone())).expect("Failed to range entities by pk");
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
            endpoint: format!("/{}/{}", entity_name.to_string().to_lowercase(), pk_name.clone()),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}