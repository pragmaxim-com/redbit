use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn by_index_def(entity_name: &Ident, entity_type: &Type, column_name: &Ident, column_type: &Type, tx_context_ty: &Type, index_table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("range_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(
            tx_context: &#tx_context_ty,
            from: &#column_type,
            until: &#column_type
        ) -> Result<Vec<#entity_type>, AppError> {
            let range_iter = tx_context.#index_table.range_keys::<#column_type>(from..until)?;
            let mut results = Vec::new();
            for entry_res in range_iter {
                let (_, mut multi_iter) = entry_res?;
                while let Some(x) = multi_iter.next() {
                    let pk = x?.value();
                    match Self::compose(&tx_context, &pk) {
                        Ok(item) => {
                            results.push(item);
                        }
                        Err(err) => {
                            return Err(AppError::Internal(err.into()));
                        }
                    }
                }
            }
            Ok(results)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = STORAGE.clone();
            let from_value = #column_type::default();
            let until_value = #column_type::default().next_value();
            let tx_context = #entity_name::begin_read_tx(&storage).expect("Failed to begin read transaction context");
            let entities = #entity_name::#fn_name(&tx_context, &from_value, &until_value).expect("Failed to get entities by range");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given range by index");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = STORAGE.clone();
            let from_value = #column_type::default();
            let until_value = #column_type::default().next_value();
            let tx_context = #entity_name::begin_read_tx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, &from_value, &until_value).expect("Failed to get entities by range");
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
