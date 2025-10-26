use crate::field_parser::EntityDef;
use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};

pub fn fn_def(entity_def: &EntityDef, table: &Ident, no_columns: bool) -> FunctionDef {
    let EntityDef { key_def, entity_name, entity_type, query_type, read_ctx_type, ..} = &entity_def;
    let pk_type = key_def.field_def().tpe;
    let fn_name = format_ident!("filter");
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#read_ctx_type, pk: #pk_type, query: &#query_type) -> Result<Option<#entity_type>, AppError> {
            if tx_context.#table.get_value(pk)?.is_some() {
                Ok(Self::compose_with_filter(&tx_context, pk, query)?)
            } else {
                Ok(None)
            }
        }
    };

    let test_with_filter_fn_name = format_ident!("{}_with_filter", fn_name);
    let filter_test = if no_columns {
        None
    } else {
        Some(quote! {
            #[test]
            fn #test_with_filter_fn_name() {
                let (storage_owner, storage) = &*STORAGE;
                let query = #query_type::sample();
                let pk_default_next = #pk_type::default().next_index();
                let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
                let entity_opt = #entity_name::#fn_name(&tx_context, pk_default_next, &query).expect("Failed to get entity by PK");
                assert_eq!(entity_opt, None, "Filter is set for default value {:?}", query);
            }
        })
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let query = #query_type::sample();
            let pk_default = #pk_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entity = #entity_name::#fn_name(&tx_context, pk_default, &query).expect("Failed to get entity by PK").expect("Expected entity to exist");
            let expected_entity = #entity_type::sample_with_query(pk_default, &query).expect("Failed to create sample entity with query");
            assert_eq!(entity, expected_entity, "Entity PK does not match the requested PK");
        }
        #filter_test
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let query = #query_type::sample();
            let pk_default = #pk_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, pk_default, &query).expect("Failed to get entity by PK").expect("Expected entity to exist");
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
