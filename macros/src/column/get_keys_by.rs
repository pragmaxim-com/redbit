use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::field_parser::EntityDef;

pub fn by_dict_def(entity_def: &EntityDef, column_name: &Ident, column_type: &Type, dict_table_var: &Ident) -> FunctionDef {
    let key_def = &entity_def.key_def.field_def();
    let pk_name = &key_def.name;
    let pk_type = &key_def.tpe;
    let fn_name = format_ident!("get_{}s_by_{}", pk_name, column_name);
    let entity_name = &entity_def.entity_name;
    let read_ctx_ty = &entity_def.read_ctx_type;
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#read_ctx_ty, val: &#column_type) -> Result<Vec<#pk_type>, AppError> {
            tx_context.#dict_table_var.get_keys(val)?.map_or(Ok(Vec::new()), redbit::utils::collect_multimap_value)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entity_pks = #entity_name::#fn_name(&tx_context, &val).expect("Failed to get entity pks by dictionary index");
            let expected_entity_pks = vec![#pk_type::default()];
            assert_eq!(expected_entity_pks, entity_pks, "Expected entity pks to be returned for the given dictionary index");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
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

pub fn by_index_def(entity_def: &EntityDef, column_name: &Ident, column_type: &Type, index_table: &Ident) -> FunctionDef {
    let entity_name = &entity_def.entity_name;
    let read_ctx_type = &entity_def.read_ctx_type;
    let key_def = &entity_def.key_def.field_def();
    let pk_name = &key_def.name;
    let pk_type = &key_def.tpe;
    let fn_name = format_ident!("get_{}s_by_{}", pk_name, column_name);
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#read_ctx_type, val: &#column_type) -> Result<Vec<#pk_type>, AppError> {
            redbit::utils::collect_multimap_value(tx_context.#index_table.get_keys(val)?)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entity_pks = #entity_name::#fn_name(&tx_context, &val).expect("Failed to get entity pks by index");
            let expected_entity_pks = vec![#pk_type::default()];
            assert_eq!(expected_entity_pks, entity_pks, "Expected entity pks to be returned for the given index");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
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