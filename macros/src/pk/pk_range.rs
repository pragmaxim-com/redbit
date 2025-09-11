use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, table_var: &Ident, tx_context_ty: &Type) -> FunctionDef {
    let fn_name = format_ident!("pk_range");
    let fn_stream = quote! {
        fn #fn_name(tx_context: &mut #tx_context_ty, from: &#pk_type, until: &#pk_type) -> Result<Vec<#pk_type>, AppError> {
            let range = from.clone()..until.clone();
            let mut iter = tx_context.#table_var.range(range)?;
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
            let storage = STORAGE.clone();
            let entity_count: usize = 3;
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next_index().next_index().next_index();
            let write_tx = storage.plain_db.begin_write().unwrap();
            let pks = {
                let mut tx_context = #entity_name::begin_write_tx(&write_tx, &storage.index_dbs).unwrap();
                let pks = #entity_name::#fn_name(&mut tx_context, &from_value, &until_value).expect("Failed to get PKs in range");
                tx_context.flush().expect("Failed to flush transaction context");
                pks
            };
            write_tx.commit().expect("Failed to commit transaction");
            let test_pks: Vec<#pk_type> = #entity_type::sample_many(entity_count).iter().map(|e| e.#pk_name).collect();
            assert_eq!(test_pks, pks, "Expected PKs to be returned for the given range");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = STORAGE.clone();
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next_index().next_index().next_index();
            b.iter(|| {
                let write_tx = storage.plain_db.begin_write().unwrap();
                {
                    let mut tx_context = #entity_name::begin_write_tx(&write_tx, &storage.index_dbs).unwrap();
                    #entity_name::#fn_name(&mut tx_context, &from_value, &until_value).expect("Failed to get PKs in range");
                    tx_context.flush().expect("Failed to flush transaction context");
                }
                write_tx.commit().expect("Failed to commit transaction");
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