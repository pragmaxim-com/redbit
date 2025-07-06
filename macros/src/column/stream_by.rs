use crate::rest::HttpParams::{FromBody, FromPath};
use crate::rest::{FunctionDef, Param, HttpMethod};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;

pub fn by_dict_def(
    entity_name: &Ident,
    entity_type: &Type,
    column_name: &Ident,
    column_type: &Type,
    value_to_dict_pk: &Ident,
    dict_index_table: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("stream_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(tx: ReadTransaction, val: #column_type) -> Result<Pin<Box<dyn futures::Stream<Item = Result<#entity_type, AppError>> + Send + 'static>>, AppError> {
            let val2birth = tx.open_table(#value_to_dict_pk)?;
            let birth_guard = val2birth.get(&val)?;

            let iter_box: Box<dyn Iterator<Item = Result<_, redb::StorageError>> + Send> = if let Some(g) = birth_guard {
                let birth_id = g.value().clone();
                let birth2pks = tx.open_multimap_table(#dict_index_table)?;
                Box::new(birth2pks.get(&birth_id)?)
            } else {
                Box::new(std::iter::empty())
            };

            let stream = futures::stream::unfold(
                (iter_box, tx),
                |(mut iter, tx)| async move {
                    match iter.next() {
                        Some(Ok(guard)) => {
                            let pk = guard.value().clone();
                            let item_res = Self::compose(&tx, &pk);
                            Some((item_res, (iter, tx)))
                        }
                        Some(Err(e)) => Some((Err(AppError::from(e)), (iter, tx))),
                        None => None,
                    }
                },
            ).boxed();

            Ok(stream)
        }
    };

    let test_stream = Some(quote! {
        {
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let entity_stream = #entity_name::#fn_name(read_tx, val).expect("Failed to get entities by dictionary index");
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given dictionary index");
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            params: vec![FromPath(vec![Param {
                name: column_name.clone(),
                ty: column_type.clone(),
                description: "Secondary index column with dictionary".to_string(),
                samples: vec![quote! { #column_type::default().encode() }],
            }])],
            method: HttpMethod::GET,
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match state.db.begin_read()
                        .map_err(AppError::from)
                        .and_then(|tx| #entity_name::#fn_name(tx, #column_name)) {
                            Ok(stream) => axum_streams::StreamBodyAs::json_nl_with_errors(stream).into_response(),
                            Err(err)   => err.into_response(),
                    }
                }
            },
            utoipa_responses: quote! { responses((status = OK, content_type = "text/event-stream", body = #entity_type)) },
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), column_name, column_name),
        }),
        test_stream
    }
}

pub fn by_index_def(entity_name: &Ident, entity_type: &Type, column_name: &Ident, column_type: &Type, table: &Ident, stream_query_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("stream_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(tx: ReadTransaction, val: #column_type, query: Option<#stream_query_type>) -> Result<Pin<Box<dyn futures::Stream<Item = Result<#entity_type, AppError>> + Send + 'static>>, AppError> {
            let mm_table = tx.open_multimap_table(#table).map_err(AppError::from)?;
            let iter = mm_table.get(&val).map_err(AppError::from)?;

            let stream = futures::stream::unfold((iter, tx, query), |(mut iter, tx, query)| async move {
                match iter.next() {
                    Some(Ok(guard)) => {
                        let pk = guard.value().clone();
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
            })
            .boxed();

            Ok(stream)
        }
    };
    let test_stream = Some(quote! {
        {
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let entity_stream = #entity_name::#fn_name(read_tx, val, None).expect("Failed to get entities by index");
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given index");
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            params: vec![
                FromPath(vec![Param {
                    name: column_name.clone(),
                    ty: column_type.clone(),
                    description: "Secondary index column".to_string(),
                    samples: vec![quote! { #column_type::default().encode() }],
                }]
                ), FromBody(Param {
                    name: format_ident!("todo"), // TODO 
                    ty: syn::parse_quote! { Option<#stream_query_type> },
                    description: "Query to filter stream entities by".to_string(),
                    samples: vec![quote! { Some(#stream_query_type::sample()) }, quote! { None }],
                })
            ],
            method: HttpMethod::POST,
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match state.db.begin_read()
                        .map_err(AppError::from)
                        .and_then(|tx| #entity_name::#fn_name(tx, #column_name, body)) {
                            Ok(stream) => axum_streams::StreamBodyAs::json_nl_with_errors(stream).into_response(),
                            Err(err)   => err.into_response(),
                    }
                }
            },
            utoipa_responses: quote! { responses((status = OK, content_type = "text/event-stream", body = #entity_type)) },
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), column_name, column_name),
        }),
        test_stream
    }
}
