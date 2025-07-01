use crate::rest::HttpParams::FromPath;
use crate::rest::{EndpointDef, FunctionDef, GetParam, HttpMethod};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

/// Generates a streaming SSE endpoint definition for querying primary keys by a dictionary index.
pub fn stream_keys_by_dict_def(
    entity_name: &Ident,
    pk_name: &Ident,
    pk_type: &Type,
    column_name: &Ident,
    column_type: &Type,
    value_to_dict_pk: &Ident,
    dict_index_table: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("stream_{}s_by_{}", pk_name, column_name);

    let fn_stream = quote! {
        pub fn #fn_name(
            tx: &::redbit::redb::ReadTransaction,
            val: &#column_type
        ) -> Result<impl redbit::futures::Stream<Item = Result<#pk_type, AppError>> + Send + 'static, AppError> {
            use redbit::futures::stream::{self, StreamExt};
            use std::iter;

            // Original dictionary lookup logic
            let val2birth = tx.open_table(#value_to_dict_pk)?;
            let birth_guard = val2birth.get(val)?;

            // Box the iterator to unify types
            let iter_box: Box<dyn Iterator<Item = Result<_, _>> + Send> = if let Some(g) = birth_guard {
                let birth_id = g.value().clone();
                let mm = tx.open_multimap_table(#dict_index_table)?;
                let it = mm.get(&birth_id)?;
                Box::new(it)
            } else {
                Box::new(iter::empty())
            };

            // Stream the iterator, mapping errors
            let stream = stream::iter(iter_box)
                .map(|res| res.map(|e| e.value().clone()).map_err(AppError::from));

            Ok(stream)
        }
    };

    let test_stream = Some(quote! {
        {
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let mut stream = #entity_name::#fn_name(&read_tx, &val).expect("Stream creation failed");
            let first = futures::executor::block_on(async { futures::StreamExt::next(&mut stream).await }).expect("Expected one item");
            assert_eq!(#pk_type::default(), first.expect("Error from stream"));
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(impl redbit::futures::Stream<Item = Result<#pk_type, AppError>> + Send + 'static),
        is_sse: true,
        fn_stream,
        fn_call: quote! { #entity_name::#fn_name(&tx, &#column_name) },
        endpoint_def: Some(EndpointDef {
            params: FromPath(vec![GetParam {
                name: column_name.clone(),
                ty: column_type.clone(),
                description: "Secondary index column (dict)".to_string(),
            }]),
            method: HttpMethod::GET,
            return_type: Some(syn::parse_quote!(#pk_type)),
            endpoint: format!("/{}/{}/{{{}}}/{}",
                              entity_name.to_string().to_lowercase(), column_name, column_name, pk_name
            ),
        }),
        test_stream: test_stream,
    }
}

/// Generates a streaming SSE endpoint definition for querying primary keys by a simple index.
pub fn stream_keys_by_index_def(
    entity_name: &Ident,
    pk_name: &Ident,
    pk_type: &Type,
    column_name: &Ident,
    column_type: &Type,
    table: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("stream_{}s_by_{}", pk_name, column_name);

    let fn_stream = quote! {
        pub fn #fn_name(
            tx: &::redbit::redb::ReadTransaction,
            val: &#column_type
        ) -> Result<impl redbit::futures::Stream<Item = Result<#pk_type, AppError>> + Send + 'static, AppError> {
            use redbit::futures::stream::{self, StreamExt};
            use std::iter;

            let it = tx.open_multimap_table(#table)?.get(val)?;
            let iter_box: Box<dyn Iterator<Item = Result<_, _>> + Send> = Box::new(it);

            let stream = stream::iter(iter_box)
                .map(|res| res.map(|e| e.value().clone()).map_err(AppError::from));

            Ok(stream)
        }
    };

    let test_stream = Some(quote! {
        {
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let mut stream = #entity_name::#fn_name(&read_tx, &val).expect("Stream creation failed");
            let first = futures::executor::block_on(async { futures::StreamExt::next(&mut stream).await }).expect("Expected one item");
            assert_eq!(#pk_type::default(), first.expect("Error from stream"));
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(impl redbit::futures::Stream<Item = Result<#pk_type, AppError>> + Send + 'static),
        is_sse: true,
        fn_stream,
        fn_call: quote! { #entity_name::#fn_name(&tx, &#column_name) },
        endpoint_def: Some(EndpointDef {
            params: FromPath(vec![GetParam {
                name: column_name.clone(),
                ty: column_type.clone(),
                description: "Secondary index column".to_string(),
            }]),
            method: HttpMethod::GET,
            return_type: Some(syn::parse_quote!(#pk_type)),
            endpoint: format!("/{}/{}/{{{}}}/{}",
                              entity_name.to_string().to_lowercase(), column_name, column_name, pk_name
            ),
        }),
        test_stream: test_stream,
    }
}
