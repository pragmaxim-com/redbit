use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn by_dict_def(
    entity_name: &Ident,
    pk_name: &Ident,
    pk_type: &Type,
    column_name: &Ident,
    column_type: &Type,
    tx_context_ty: &Type,
    dict_table_var: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("get_{}s_by_{}", pk_name, column_name);
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#tx_context_ty, val: &#column_type) -> Result<Vec<#pk_type>, AppError> {
            let iter_opt = tx_context.#dict_table_var.get_keys(val)?;
            let results =
                match iter_opt {
                    None => Vec::new(),
                    Some(mut iter) => {
                        let mut results = Vec::new();
                        while let Some(x) = iter.next() {
                            let pk = x?.value();
                            results.push(pk);
                        }
                        results
                    },
                };
            Ok(results)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = STORAGE.clone();
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_tx(&storage).expect("Failed to begin read transaction context");
            let entity_pks = #entity_name::#fn_name(&tx_context, &val).expect("Failed to get entity pks by dictionary index");
            let expected_entity_pks = vec![#pk_type::default()];
            assert_eq!(expected_entity_pks, entity_pks, "Expected entity pks to be returned for the given dictionary index");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = STORAGE.clone();
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_tx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, &val).expect("Failed to get entity pks by dictionary index");
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

pub fn by_index_def(entity_name: &Ident, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type, tx_context_ty: &Type, index_table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("get_{}s_by_{}", pk_name, column_name);
    let fn_stream = quote! {
        pub fn #fn_name(
            tx_context: &#tx_context_ty,
            val: &#column_type
        ) -> Result<Vec<#pk_type>, AppError> {
            let mut iter = tx_context.#index_table.get_keys(val)?;
            let mut results = Vec::new();
            while let Some(x) = iter.next() {
                let pk = x?.value();
                results.push(pk);
            }
            Ok(results)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = STORAGE.clone();
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_tx(&storage).expect("Failed to begin read transaction context");
            let entity_pks = #entity_name::#fn_name(&tx_context, &val).expect("Failed to get entity pks by index");
            let expected_entity_pks = vec![#pk_type::default()];
            assert_eq!(expected_entity_pks, entity_pks, "Expected entity pks to be returned for the given index");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = STORAGE.clone();
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_tx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, &val).expect("Failed to get entity pks by index");
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