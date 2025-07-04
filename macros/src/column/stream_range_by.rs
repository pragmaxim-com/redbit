use crate::rest::HttpParams::FromQuery;
use crate::rest::{FunctionDef, HttpMethod};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;

pub fn stream_range_by_index_def(entity_name: &Ident, entity_type: &Type, column_name: &Ident, column_type: &Type, table: &Ident, column_query: Type) -> FunctionDef {
    let fn_name = format_ident!("stream_range_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(
            tx: ReadTransaction,
            from: #column_type,
            until: #column_type,
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
            Ok(pk_stream.try_flatten().map(move |pk_res| pk_res.and_then(|pk| Self::compose(&tx, &pk))).boxed())
        }
    };
    let test_stream =  Some(quote! {
        {
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let from_value = #column_type::default();
            let until_value = #column_type::default().next();
            let entity_stream = #entity_name::#fn_name(read_tx, from_value, until_value).expect("Failed to range entities by index");
            let entities = entity_stream.try_collect::<Vec<#entity_type>>().await.expect("Failed to collect entity stream");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given stream_range by index");
        }
    });
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            params: vec![FromQuery(column_query)],
            method: HttpMethod::GET,
            handler_impl_stream: quote! {
               impl IntoResponse {
                   match state.db.begin_read()
                        .map_err(AppError::from)
                        .and_then(|tx| #entity_name::#fn_name(tx, query.from, query.until)) {
                            Ok(stream) => axum_streams::StreamBodyAs::json_nl_with_errors(stream).into_response(),
                            Err(err)   => err.into_response(),
                    }
                }
            },
            utoipa_responses: quote! { responses((status = OK, content_type = "text/event-stream", body = #entity_type)) },
            endpoint: format!("/{}/{}", entity_name.to_string().to_lowercase(), column_name.clone()),
        }),
        test_stream
    }
}
