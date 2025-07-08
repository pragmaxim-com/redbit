use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_type: &Type, table: &Ident, stream_query_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("range");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx: &ReadTransaction, from: &#pk_type, until: &#pk_type, query: Option<#stream_query_type>) -> Result<Vec<#entity_type>, AppError> {
                let table_pk_9 = tx.open_table(#table)?;
                let range = from..until;
                let mut iter = table_pk_9.range::<#pk_type>(range)?;
                let mut results = Vec::new();
                if let Some(ref q) = query {
                    while let Some(entry_res) = iter.next() {
                        let pk = entry_res?.0.value();
                        match Self::compose_with_filter(&tx, &pk, q)? {
                            Some(entity) => results.push(entity),
                            None => {},
                        }
                    }
                } else {
                    while let Some(entry_res) = iter.next() {
                        let pk = entry_res?.0.value();
                        results.push(Self::compose(&tx, &pk)?);
                    }
                }
                Ok(results)
            }
        };
    let test_fn_name = format_ident!("test_{}", fn_name);
    let test_with_filter_fn_name = format_ident!("{}_with_filter", test_fn_name);
    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #test_fn_name() {
            let db = DB.clone();
            let entity_count: usize = 3;
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next().next();
            let entities = #entity_name::#fn_name(&read_tx, &from_value, &until_value, None).expect("Failed to get entities by range");
            let expected_entities = #entity_type::sample_many(2);
            assert_eq!(entities, expected_entities, "Expected entities to be returned for the given range");
        }
        #[tokio::test]
        async fn #test_with_filter_fn_name() {
            let db = DB.clone();
            let entity_count: usize = 3;
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next().next().next();
            let query = #stream_query_type::sample();
            let entities = #entity_name::#fn_name(&read_tx, &from_value, &until_value, Some(query.clone())).expect("Failed to get entities by range");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(entities, expected_entities, "Only the default valued entity, filter is set for default values, query: {:?}", query);
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: None,
        test_stream
    }
}