use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::table::DictTableDefs;

pub fn get_by_dict_def(
    entity_name: &Ident,
    entity_type: &Type,
    column_name: &Ident,
    column_type: &Type,
    tx_context_ty: &Type,
    dict_table_defs: &DictTableDefs,
) -> FunctionDef {
    let value_to_dict_pk = &dict_table_defs.value_to_dict_pk_table_def.var_name;
    let dict_index_table = &dict_table_defs.dict_index_table_def.var_name;

    let fn_name = format_ident!("get_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#tx_context_ty, val: &#column_type) -> Result<Vec<#entity_type>, AppError> {
            let birth_guard = tx_context.#value_to_dict_pk.get(val)?;
            let birth_id = match birth_guard {
                Some(g) => g.value(),
                None => return Ok(Vec::new()),
            };
            let mut iter = tx_context.#dict_index_table.get(&birth_id)?;
            let mut results = Vec::new();
            while let Some(x) = iter.next() {
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
            Ok(results)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = STORAGE.clone();
            let val = #column_type::default();
            let read_tx = storage.db.begin_read().expect("Failed to begin read transaction");
            let tx_context = #entity_name::begin_read_tx(&read_tx).expect("Failed to begin read transaction context");
            let entities = #entity_name::#fn_name(&tx_context, &val).expect("Failed to get entities by dictionary index");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given dictionary index");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = STORAGE.clone();
            let read_tx = storage.db.begin_read().expect("Failed to begin read transaction");
            let tx_context = #entity_name::begin_read_tx(&read_tx).expect("Failed to begin read transaction context");
            let val = #column_type::default();
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, &val).expect("Failed to get entities by dictionary index");
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

pub fn get_by_index_def(entity_name: &Ident, entity_type: &Type, column_name: &Ident, column_type: &Type, tx_context_ty: &Type, table_var: &Ident) -> FunctionDef {
    let fn_name = format_ident!("get_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#tx_context_ty, val: &#column_type) -> Result<Vec<#entity_type>, AppError> {
            let mut iter = tx_context.#table_var.get(val)?;
            let mut results = Vec::new();
            while let Some(x) = iter.next() {
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
            Ok(results)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = STORAGE.clone();
            let val = #column_type::default();
            let read_tx = storage.db.begin_read().expect("Failed to begin read transaction");
            let tx_context = #entity_name::begin_read_tx(&read_tx).expect("Failed to begin read transaction context");
            let entities = #entity_name::#fn_name(&tx_context, &val).expect("Failed to get entities by index");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given index");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = STORAGE.clone();
            let val = #column_type::default();
            let read_tx = storage.db.begin_read().expect("Failed to begin read transaction");
            let tx_context = #entity_name::begin_read_tx(&read_tx).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, &val).expect("Failed to get entities by index");
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
