use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;
use crate::field_parser::EntityDef;
use crate::rest::FunctionDef;

pub fn compose_token_stream(entity_def: &EntityDef, field_names: &[Ident], struct_inits: &[TokenStream]) -> FunctionDef {
    let entity_name = &entity_def.entity_name;
    let entity_type = &entity_def.entity_type;
    let pk_type: &Type = &entity_def.key_def.field_def().tpe;
    let read_ctx_type: &Type = &entity_def.read_ctx_type;

    FunctionDef {
        fn_stream: quote! {
            fn compose(tx_context: &#read_ctx_type, pk: #pk_type) -> Result<#entity_type, AppError> {
                #(#struct_inits)*
                Ok(#entity_type {
                    #(#field_names,)*
                })
            }
        },
        endpoint: None,
        test_stream: Some(quote! {
            #[test]
            fn compose_valid_entity() -> Result<(), AppError> {
                let (storage_owner, storage) = random_storage();
                let pk = #pk_type::default();
                let sample_entity = #entity_name::sample();
                let write_result = #entity_name::persist(Arc::clone(&storage), sample_entity);
                assert!(write_result.is_ok());
                let tx_context = #entity_name::begin_read_ctx(&storage)?;
                let entity = #entity_name::compose(&tx_context, pk)?;
                let loaded_entity = #entity_name::get(&tx_context, pk)?.expect("Entity not found");
                let serialization_result = serde_json::to_string(&loaded_entity);
                assert!(serialization_result.is_ok(), "Failed to serialize entity to JSON");
                Ok(())
            }
        }),
        bench_stream: None,
    }
}

pub fn compose_with_filter_token_stream(entity_def: &EntityDef, field_names: &[Ident], struct_inits_with_query: &[TokenStream]) -> FunctionDef {
    let EntityDef { key_def, entity_type, query_type, read_ctx_type, ..} = &entity_def;
    let pk_type: &Type = &key_def.field_def().tpe;
    FunctionDef {
        fn_stream: quote! {
            fn compose_with_filter(tx_context: &#read_ctx_type, pk: #pk_type, stream_query: &#query_type) -> Result<Option<#entity_type>, AppError> {
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
            fn compose_with_filter_valid_entity() -> Result<(), AppError> {
                let (storage_owner, storage) = random_storage();
                let pk = #pk_type::default();
                let sample_entity = #entity_type::sample();
                let write_result = #entity_type::persist(Arc::clone(&storage), sample_entity);
                let query = #query_type::default();
                let tx_context = #entity_type::begin_read_ctx(&storage)?;
                let entity = #entity_type::compose_with_filter(&tx_context, pk, &query)?.expect("Entity does not match");
                assert!(write_result.is_ok());
                let loaded_entity = #entity_type::get(&tx_context, pk)?.expect("Entity not found");
                let serialization_result = serde_json::to_string(&loaded_entity);
                assert!(serialization_result.is_ok(), "Failed to serialize entity to JSON");
                Ok(())
            }
        }),
        bench_stream: None,
    }
}

pub fn compose_many_token_stream(entity_def: &EntityDef) -> FunctionDef {
    let pk_type: &Type = &entity_def.key_def.field_def().tpe;
    let EntityDef { entity_type, query_type, read_ctx_type, ..} = &entity_def;
    FunctionDef {
        fn_stream: quote! {
            fn compose_many<I: Iterator<Item = redb::Result<#pk_type>>>(
                tx_context: &#read_ctx_type,
                pk_values: I,
                stream_query: Option<#query_type>
            ) -> Result<Vec<#entity_type>, AppError> {
                let mut results = Vec::with_capacity(pk_values.size_hint().0);
                match stream_query {
                    Some(ref q) => {
                        for pk in pk_values {
                            if let Ok(Some(item)) = Self::compose_with_filter(&tx_context, pk?, q) {
                                results.push(item);
                            }
                        }
                    }
                    None => {
                        for pk in pk_values {
                            results.push(Self::compose(&tx_context, pk?)?);
                        }
                    }
                }
                Ok(results)
            }
        },
        endpoint: None,
        test_stream: None,
        bench_stream: None,
    }
}

pub fn compose_many_stream_token_stream(entity_def: &EntityDef) -> FunctionDef {
    let pk_type: &Type = &entity_def.key_def.field_def().tpe;
    let EntityDef { entity_type, query_type, read_ctx_type, ..} = &entity_def;
    FunctionDef {
        fn_stream: quote! {
            pub fn compose_many_stream<I: Iterator<Item = redb::Result<#pk_type>> + Send>(
                tx_context: #read_ctx_type,
                pk_values: I,
                stream_query: Option<#query_type>,
            ) -> Result<impl futures::Stream<Item = Result<#entity_type, AppError>> + Send, AppError> {
                let iter = pk_values.filter_map(move |item_res| {
                    match item_res {
                        Err(e) => Some(Err(AppError::from(e))),
                        Ok(pk) => {
                            if let Some(ref q) = stream_query {
                                match Self::compose_with_filter(&tx_context, pk, q) {
                                    Ok(Some(item)) => Some(Ok(item)),
                                    Ok(None) => None, // skip
                                    Err(err) => Some(Err(AppError::Internal(err.into()))),
                                }
                            } else {
                                match Self::compose(&tx_context, pk) {
                                    Ok(item) => Some(Ok(item)),
                                    Err(err) => Some(Err(AppError::Internal(err.into()))),
                                }
                            }
                        }
                    }
                });
                Ok(stream::iter(iter))
            }
        },
        endpoint: None,
        test_stream: None,
        bench_stream: None,
    }
}

