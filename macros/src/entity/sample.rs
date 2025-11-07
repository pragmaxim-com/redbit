use crate::field_parser::EntityDef;
use crate::rest::FunctionDef;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

pub fn sample_token_fns(
    entity_def: &EntityDef,
    struct_default_inits: &[TokenStream],
    struct_default_inits_with_query: &[TokenStream],
    field_names: &[Ident],
) -> Vec<FunctionDef> {
    let EntityDef { key_def, entity_name, entity_type, query_type, ..} = &entity_def;
    let pk_type: &Type = &key_def.field_def().tpe;
    vec![
        FunctionDef {
            fn_stream: quote! {
                pub fn sample() -> Self {
                    #entity_name::sample_with(#pk_type::default())
                }
            },
            endpoint: None,
            test_stream: Some(quote! {
                #[test]
                fn sample_json() -> Result<(), AppError> {
                    let entity = #entity_name::sample();
                    serde_json::to_string(&entity)?;
                    Ok(())
                }
            }),
            bench_stream: None
        },
        FunctionDef {
            fn_stream: quote! {
                pub fn sample_many(pk: #pk_type, n: usize) -> Vec<#entity_type> {
                    let first = #entity_type::sample_with(pk);
                    std::iter::successors(Some((pk.next_index(), Some(first))), |&(prev_pointer, _)| {
                        let new_entity = #entity_type::sample_with(prev_pointer);
                        Some((prev_pointer.next_index(), Some(new_entity)))
                    })
                    .filter_map(|(_, instance)| instance)
                    .take(n)
                    .collect()
                }
            },
            endpoint: None,
            test_stream: Some(quote! {
                #[test]
                fn sample_many_json() -> Result<(), AppError> {
                    let entities = #entity_name::sample_many(Default::default(), 3);
                    serde_json::to_string(&entities)?;
                    Ok(())
                }
            }),
            bench_stream: None
        },
        FunctionDef {
            fn_stream: quote! {
                pub fn sample_many_with_query(pk: #pk_type, stream_query: &#query_type, n: usize) -> Vec<#entity_type> {
                    let first: Option<#entity_type> = #entity_type::sample_with_query(pk, stream_query);
                    std::iter::successors(first.map(|e| (pk.next_index(), e)), |&(prev_pointer, _)| {
                        #entity_type::sample_with_query(prev_pointer, stream_query).map(|new_entity| (prev_pointer.next_index(), new_entity))
                    })
                    .map(|(_, instance)| instance) // now safe, always Some
                    .take(n)
                    .collect()
                }
            },
            endpoint: None,
            test_stream: Some(quote! {
                #[test]
                fn sample_many_with_query_json() -> Result<(), AppError> {
                    let query = #query_type::sample();
                    let entities = #entity_name::sample_many_with_query(Default::default(), &query, 3);
                    serde_json::to_string(&entities)?;
                    Ok(())
                }
            }),
            bench_stream: None
        },
        FunctionDef {
            fn_stream: quote! {
                pub fn sample_with(pk: #pk_type) -> Self {
                    #(#struct_default_inits)*
                    #entity_type {
                        #(#field_names,)*
                    }
                }
            },
            endpoint: None,
            test_stream: None,
            bench_stream: None
        },
        FunctionDef {
            fn_stream: quote! {
                pub fn sample_with_query(pk: #pk_type, stream_query: &#query_type) -> Option<#entity_type> {
                    // First: fetch & filter every column, short‑circuit on mismatch
                    #(#struct_default_inits_with_query)*
                    Some(
                        #entity_type {
                            #(#field_names,)*
                        }
                    )
                }
            },
            endpoint: None,
            test_stream: Some(quote! {
                #[test]
                fn sample_with_query_json() -> Result<(), AppError> {
                    let pk = #pk_type::default();
                    let query = #query_type::sample();
                    let entity = #entity_name::sample_with_query(pk, &query);
                    serde_json::to_string(&entity)?;
                    Ok(())
                }
            }),
            bench_stream: None
        },
    ]
}
