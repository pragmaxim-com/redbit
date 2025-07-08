use crate::rest::HttpParams::{FromBody, FromQuery};
use crate::rest::{FunctionDef, HttpMethod, Param};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, table: &Ident, column_query: Type, stream_query_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("stream_range");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx: ReadTransaction, from: #pk_type, until: #pk_type, query: Option<#stream_query_type>) -> Result<Pin<Box<dyn futures::Stream<Item = Result<#entity_type, AppError>> + Send + 'static>>, AppError> {
                if from >= until {
                    return Err(AppError::BadRequest("Range cannot be empty".to_string()));
                }
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
    let test_fn_name = format_ident!("test_{}", fn_name);
    let test_with_filter_fn_name = format_ident!("{}_with_filter", test_fn_name);
    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #test_fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next().next();
            let entity_stream = #entity_name::#fn_name(read_tx, from_value, until_value, None).expect("Failed to range entities by pk");
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
            let expected_entities = #entity_type::sample_many(2);
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given range");
        }
        #[tokio::test]
        async fn #test_with_filter_fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next().next().next();
            let query = #stream_query_type::sample();
            let entity_stream = #entity_name::#fn_name(read_tx, from_value, until_value, Some(query.clone())).expect("Failed to range entities by pk");
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(entities, expected_entities, "Only the default valued entity, filter is set for default values, query: {:?}", query);
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            params: vec![FromQuery(Param {
                name: format_ident!("query"), // TODO
                ty: column_query.clone(),
                description: "Range query from/until".to_string(),
                samples: quote! { vec![#column_query::sample()] },
            }), FromBody(Param {
                name: format_ident!("todo"), // TODO
                ty: syn::parse_quote! { Option<#stream_query_type> },
                description: "Query to filter stream entities by".to_string(),
                samples: quote! { vec![Some(#stream_query_type::sample()), None ] },
            })],
            method: HttpMethod::POST,
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
            utoipa_responses: quote! { responses((status = OK, content_type = "text/event-stream", body = #entity_type)) },
            endpoint: format!("/{}/{}", entity_name.to_string().to_lowercase(), pk_name.clone()),
        }),
        test_stream
    }
}