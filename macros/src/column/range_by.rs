use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::field_parser::EntityDef;

pub fn by_index_def(entity_def: &EntityDef, column_name: &Ident, column_type: &Type, index_table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("range_by_{}", column_name);
    let entity_name = &entity_def.entity_name;
    let entity_type = &entity_def.entity_type;
    let read_ctx_type = &entity_def.read_ctx_type;
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#read_ctx_type, from: &#column_type, until: &#column_type) -> Result<Vec<#entity_type>, AppError> {
            let iter = tx_context.#index_table.index_range::<#column_type>(from..until)?
                .flat_map(|r| match r {
                    Ok((_k, value_iter)) => Either::Left(value_iter.map(|res| res.map(|kg| kg.value()))),
                    Err(e) => Either::Right(std::iter::once(Err(e))),
                });
            Self::compose_many(&tx_context, iter, None)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let from_value = #column_type::default();
            let until_value = #column_type::default().next_value();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entities = #entity_name::#fn_name(&tx_context, &from_value, &until_value).expect("Failed to get entities by range");
            let expected_entities = vec![#entity_type::sample()];
            assert_eq!(expected_entities, entities, "Expected entities to be returned for the given range by index");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let from_value = #column_type::default();
            let until_value = #column_type::default().next_value();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, &from_value, &until_value).expect("Failed to get entities by range");
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
