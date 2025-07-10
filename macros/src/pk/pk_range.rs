use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("pk_range");
    let fn_stream = quote! {
        fn #fn_name(tx: &WriteTransaction, from: &#pk_type, until: &#pk_type) -> Result<Vec<#pk_type>, AppError> {
            let table_pk_10 = tx.open_table(#table)?;
            let range = from.clone()..until.clone();
            let mut iter = table_pk_10.range(range)?;
            let mut results = Vec::new();
            while let Some(entry_res) = iter.next() {
                let pk = entry_res?.0.value();
                results.push(pk);
            }
            Ok(results)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let db = DB.clone();
            let entity_count: usize = 3;
            let write_tx = db.begin_write().expect("Failed to begin write transaction");
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next().next().next();
            let pks = #entity_name::#fn_name(&write_tx, &from_value, &until_value).expect("Failed to get PKs in range");
            let test_pks: Vec<#pk_type> = #entity_type::sample_many(entity_count).iter().map(|e| e.#pk_name.clone()).collect();
            assert_eq!(test_pks, pks, "Expected PKs to be returned for the given range");
        }
    });

    let bench_fn_name = format_ident!("bench_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let db = DB.clone();
            let write_tx = db.begin_write().expect("Failed to begin write transaction");
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next().next().next();
            b.iter(|| {
                #entity_name::#fn_name(&write_tx, &from_value, &until_value).expect("Failed to get PKs in range");
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