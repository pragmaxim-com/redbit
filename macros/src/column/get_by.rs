use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::field_parser::EntityDef;

pub fn get_by_dict_def(
    entity_def: &EntityDef,
    column_name: &Ident,
    column_type: &Type,
    dict_table_var: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("get_by_{}", column_name);
    let entity_name = &entity_def.entity_name;
    let entity_type = &entity_def.entity_type;
    let read_tx_context_ty = &entity_def.read_ctx_type;
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#read_tx_context_ty, val: &#column_type) -> Result<Vec<#entity_type>, AppError> {
            let iter = tx_context.#dict_table_var.get_keys(val)?.into_iter().flatten().map(|res| res.map(|kg| kg.value()));
            Self::compose_many(&tx_context, iter, None)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entities = #entity_name::#fn_name(&tx_context, &val).expect("Failed to get entities by dictionary index");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given dictionary index");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
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

pub fn get_by_index_def(entity_def: &EntityDef, column_name: &Ident, column_type: &Type, index_table_var: &Ident) -> FunctionDef {
    let fn_name = format_ident!("get_by_{}", column_name);
    let entity_name = &entity_def.entity_name;
    let entity_type = &entity_def.entity_type;
    let read_ctx_type = &entity_def.read_ctx_type;
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#read_ctx_type, val: &#column_type) -> Result<Vec<#entity_type>, AppError> {
            let iter = tx_context.#index_table_var.get_keys(val)?.map(|res| res.map(|kg| kg.value()));
            Self::compose_many(&tx_context, iter, None)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let val = #column_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entities = #entity_name::#fn_name(&tx_context, &val).expect("Failed to get entities by index");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given index");
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
