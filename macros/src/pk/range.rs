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

    let test_with_filter_fn_name = format_ident!("{}_with_filter", fn_name);
    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next_index().next_index();
            let entities = #entity_name::#fn_name(&read_tx, &from_value, &until_value, None).expect("Failed to get entities by range");
            let expected_entities = #entity_type::sample_many(2);
            assert_eq!(entities, expected_entities, "Expected entities to be returned for the given range");
        }
        #[test]
        fn #test_with_filter_fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next_index().next_index().next_index();
            let query = #stream_query_type::sample();
            let entities = #entity_name::#fn_name(&read_tx, &from_value, &until_value, Some(query.clone())).expect("Failed to get entities by range");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(entities, expected_entities, "Only the default valued entity, filter is set for default values, query: {:?}", query);
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next_index().next_index().next_index();
            let query = #stream_query_type::sample();
            b.iter(|| {
                #entity_name::#fn_name(&read_tx, &from_value, &until_value, Some(query.clone())).expect("Failed to get entities by range");
            });
        }
    });


    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream,
        bench_stream
    }
}