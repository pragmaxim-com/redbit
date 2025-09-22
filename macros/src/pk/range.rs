use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_type: &Type, tx_context_ty: &Type, table: &Ident, stream_query_type: &Type, no_columns: bool) -> FunctionDef {
    let fn_name = format_ident!("range");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx_context: &#tx_context_ty, from: &#pk_type, until: &#pk_type, query: Option<#stream_query_type>) -> Result<Vec<#entity_type>, AppError> {
                let range = from..until;
                let mut iter = tx_context.#table.range::<#pk_type>(range)?;
                let mut results = Vec::new();
                if let Some(ref q) = query {
                    while let Some(entry_res) = iter.next() {
                        let pk = entry_res?.0.value();
                        match Self::compose_with_filter(&tx_context, &pk, q)? {
                            Some(entity) => results.push(entity),
                            None => {},
                        }
                    }
                } else {
                    while let Some(entry_res) = iter.next() {
                        let pk = entry_res?.0.value();
                        results.push(Self::compose(&tx_context, &pk)?);
                    }
                }
                Ok(results)
            }
        };

    let test_with_filter_fn_name = format_ident!("{}_with_filter", fn_name);
    let test_stream_with_filter = if no_columns {
        None
    } else {
        Some(quote! {
            #[test]
            fn #test_with_filter_fn_name() {
                let (storage_owner, storage) = &*STORAGE;
                let pk = #pk_type::default();
                let from_value = #pk_type::default();
                let until_value = #pk_type::default().next_index().next_index().next_index();
                let query = #stream_query_type::sample();
                let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
                let entities = #entity_name::#fn_name(&tx_context, &from_value, &until_value, Some(query.clone())).expect("Failed to get entities by range");
                let expected_entity = #entity_type::sample_with_query(&pk, 0, &query).expect("Failed to create sample entity with query");
                assert_eq!(entities.len(), 1, "Expected only one entity to be returned for the given range with filter");
                assert_eq!(entities[0], expected_entity, "Range result is not equal to sample because it is filtered, query: {:?}", query);
            }
        })
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next_index().next_index();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entities = #entity_name::#fn_name(&tx_context, &from_value, &until_value, None).expect("Failed to get entities by range");
            let expected_entities = #entity_type::sample_many(2);
            assert_eq!(entities, expected_entities, "Expected entities to be returned for the given range");
        }
        #test_stream_with_filter
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next_index().next_index().next_index();
            let query = #stream_query_type::sample();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, &from_value, &until_value, Some(query.clone())).expect("Failed to get entities by range");
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