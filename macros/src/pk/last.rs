use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("last");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &ReadTransaction) -> Result<Option<#entity_type>, AppError> {
            let table_pk_8 = tx.open_table(#table)?;
            if let Some((k, _)) = table_pk_8.last()? {
                return Self::compose(&tx, &k.value()).map(Some);
            }
            Ok(None)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let db = DB.clone();
            let entity_count: usize = 3;
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let entity = #entity_name::last(&read_tx).expect("Failed to get last entity by PK").expect("Expected last entity to exist");
            let expected_entity = #entity_type::sample_many(entity_count).last().expect("Expected at least one entity").clone();
            assert_eq!(entity, expected_entity, "Last entity does not match expected");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            b.iter(|| {
                #entity_name::last(&read_tx).expect("Failed to get last entity by PK").expect("Expected last entity to exist");
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