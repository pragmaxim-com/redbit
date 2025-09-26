use crate::field_parser::EntityDef;
use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};

pub fn fn_def(entity_def: &EntityDef, table_var: &Ident) -> FunctionDef {
    let EntityDef { key_def, entity_name, entity_type, query_type: _, info_type:_, read_ctx_type:_ , write_ctx_type} = &entity_def;
    let fn_name = format_ident!("pk_range");
    let key_def = key_def.field_def();
    let pk_name = &key_def.name;
    let pk_type = &key_def.tpe;

    let fn_stream = quote! {
        fn #fn_name(tx_context: &#write_ctx_type, from: #pk_type, until: #pk_type) -> Result<Vec<#pk_type>, AppError> {
            let entries = tx_context.#table_var.range(from, until)?;
            let mut results = Vec::new();
            for (key, _) in entries {
                let pointer: #pk_type = key.as_value();
                results.push(pointer);
            }
            Ok(results)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let entity_count: usize = 3;
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next_index().next_index().next_index();
            let pks = {
                let tx_context = #entity_name::begin_write_ctx(&storage).unwrap();
                let pks = #entity_name::#fn_name(&tx_context, from_value, until_value).expect("Failed to get PKs in range");
                tx_context.commit_and_close_ctx().expect("Failed to flush transaction context");
                pks
            };
            let test_pks: Vec<#pk_type> = #entity_type::sample_many(entity_count).iter().map(|e| e.#pk_name).collect();
            assert_eq!(test_pks, pks, "Expected PKs to be returned for the given range");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next_index().next_index().next_index();
            let tx_context = #entity_name::new_write_ctx(&storage).unwrap();
            b.iter(|| {
                let _ = tx_context.begin_writing().expect("Failed to begin writing");
                #entity_name::#fn_name(&tx_context, from_value, until_value).expect("Failed to get PKs in range");
                let _ = tx_context.two_phase_commit().expect("Failed to commit");
            });
            tx_context.stop_writing().unwrap();
        }
    });

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream,
        bench_stream
    }

}