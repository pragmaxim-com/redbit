use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;
use crate::rest::FunctionDef;

pub fn compose_token_stream(entity_name: &Ident, entity_type: &Type, pk_type: &Type, struct_inits: &[TokenStream]) -> FunctionDef {
    FunctionDef {
        fn_stream: quote! {
            fn compose(tx: &StorageReadTx, pk: &#pk_type) -> Result<#entity_type, AppError> {
                Ok(#entity_name {
                    #(#struct_inits),*
                })
            }
        },
        endpoint: None,
        test_stream: Some(quote! {
            #[test]
            fn compose_valid_entity() {
                let storage = random_storage();
                let pk = #pk_type::default();
                let sample_entity = #entity_name::sample();
                let write_result = #entity_name::store_and_commit(Arc::clone(&storage), &sample_entity);
                let tx = storage.begin_read().unwrap();
                let entity = #entity_name::compose(&tx, &pk).unwrap();
                assert!(write_result.is_ok());
                let loaded_entity = #entity_name::get(&tx, &pk).expect("Failed to load entity").expect("Entity not found");
                let serialization_result = serde_json::to_string(&loaded_entity);
                assert!(serialization_result.is_ok(), "Failed to serialize entity to JSON");
            }
        }),
        bench_stream: None,
    }
}

pub fn compose_with_filter_token_stream(entity_type: &Type, pk_type: &Type, stream_query_type: &Type, field_names: &[Ident], struct_inits_with_query: &[TokenStream]) -> FunctionDef {
    FunctionDef {
        fn_stream: quote! {
            fn compose_with_filter(tx: &StorageReadTx, pk: &#pk_type, stream_query: &#stream_query_type) -> Result<Option<#entity_type>, AppError> {
                // First: fetch & filter every column, short‑circuit on mismatch
                #(#struct_inits_with_query)*
                Ok(Some(#entity_type {
                    #(#field_names,)*
                }))
            }
        },
        endpoint: None,
        test_stream: Some(quote! {
            #[test]
            fn compose_with_filter_valid_entity() {
                let storage = random_storage();
                let pk = #pk_type::default();
                let sample_entity = #entity_type::sample();
                let write_result = #entity_type::store_and_commit(Arc::clone(&storage), &sample_entity);
                let query = #stream_query_type::default();
                let tx = storage.begin_read().unwrap();
                let entity = #entity_type::compose_with_filter(&tx, &pk, &query).expect("Failed to compose entity").expect("Entity does not match");
                assert!(write_result.is_ok());
                let loaded_entity = #entity_type::get(&tx, &pk).expect("Failed to load entity").expect("Entity not found");
                let serialization_result = serde_json::to_string(&loaded_entity);
                assert!(serialization_result.is_ok(), "Failed to serialize entity to JSON");
            }
        }),
        bench_stream: None,
    }

}
