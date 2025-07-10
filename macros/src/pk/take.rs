use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("take");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &ReadTransaction, n: usize) -> Result<Vec<#entity_type>, AppError> {
            let table_pk_6 = tx.open_table(#table)?;
            let mut iter = table_pk_6.iter()?;
            let mut results = Vec::new();
            let mut count: usize = 0;
            if n > 100 {
                return Err(AppError::Internal("Cannot take more than 100 entities at once".to_string()));
            } else {
                while let Some(entry_res) = iter.next() {
                    if count >= n {
                        break;
                    }
                    let pk = entry_res?.0.value();
                    results.push(Self::compose(&tx, &pk)?);
                    count += 1;
                }
                Ok(results)
            }
        }
    };

    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let n: usize = 2;
            let entities = #entity_name::#fn_name(&read_tx, n).expect("Failed to take entities");
            let expected_entities = #entity_type::sample_many(n);
            assert_eq!(entities, expected_entities, "Expected to take 2 entities");
        }
    });

    let bench_fn_name = format_ident!("bench_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let n: usize = 2;
            b.iter(|| {
                #entity_name::#fn_name(&read_tx, n).expect("Failed to take entities");
            });
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: None,
        test_stream,
        bench_stream,
    }
}
