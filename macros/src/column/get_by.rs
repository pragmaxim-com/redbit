use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn get_by_dict_def(
    entity_name: &Ident,
    entity_type: &Type,
    column_name: &Ident,
    column_type: &Type,
    value_to_dict_pk: &Ident,
    dict_index_table: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("get_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(tx: &ReadTransaction, val: &#column_type) -> Result<Vec<#entity_type>, AppError> {
            let val2birth = tx.open_table(#value_to_dict_pk)?;
            let birth_guard = val2birth.get(val)?;
            let birth_id = match birth_guard {
                Some(g) => g.value().clone(),
                None => return Ok(Vec::new()),
            };
            let birth2pks = tx.open_multimap_table(#dict_index_table)?;
            let mut iter = birth2pks.get(&birth_id)?;
            let mut results = Vec::new();
            while let Some(x) = iter.next() {
                let pk = x?.value();
                match Self::compose(&tx, &pk) {
                    Ok(item) => {
                        results.push(item);
                    }
                    Err(err) => {
                        return Err(AppError::Internal(err.to_string()));
                    }
                }
            }
            Ok(results)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let entities = #entity_name::#fn_name(&read_tx, &val).expect("Failed to get entities by dictionary index");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given dictionary index");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            b.iter(|| {
                #entity_name::#fn_name(&read_tx, &val).expect("Failed to get entities by dictionary index");
            });
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: None,
        test_stream,
        bench_stream
    }
}

pub fn get_by_index_def(entity_name: &Ident, entity_type: &Type, column_name: &Ident, column_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("get_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(tx: &ReadTransaction, val: &#column_type) -> Result<Vec<#entity_type>, AppError> {
            let mm_table = tx.open_multimap_table(#table)?;
            let mut iter = mm_table.get(val)?;
            let mut results = Vec::new();
            while let Some(x) = iter.next() {
                let pk = x?.value();
                match Self::compose(&tx, &pk) {
                    Ok(item) => {
                        results.push(item);
                    }
                    Err(err) => {
                        return Err(AppError::Internal(err.to_string()));
                    }
                }
            }
            Ok(results)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let entities = #entity_name::#fn_name(&read_tx, &val).expect("Failed to get entities by index");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given index");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            b.iter(|| {
                #entity_name::#fn_name(&read_tx, &val).expect("Failed to get entities by index");
            });
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: None,
        test_stream,
        bench_stream
    }
}
