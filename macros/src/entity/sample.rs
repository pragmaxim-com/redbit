use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;
use crate::rest::FunctionDef;

pub fn sample_token_fns(
    entity_name: &Ident,
    entity_type: &Type,
    pk_type: &Type,
    stream_query_type: &Type,
    struct_default_inits: &[TokenStream],
    struct_default_inits_with_query: &[TokenStream],
    field_names: &[Ident],
) -> Vec<FunctionDef> {
    vec![
        FunctionDef {
            fn_stream: quote! {
                pub fn sample() -> Self {
                    #entity_name::sample_with(#pk_type::default(), 0)
                }
            },
            endpoint: None,
            test_stream: Some(quote! {
                #[test]
                fn sample_json() {
                    let entity = #entity_name::sample();
                    serde_json::to_string(&entity).expect("Failed to serialize sample entity to JSON");
                }
            }),
            bench_stream: None
        },
        FunctionDef {
            fn_stream: quote! {
                pub fn sample_many(n: usize) -> Vec<#entity_type> {
                    let mut sample_index = 0;
                    std::iter::successors(Some((#pk_type::default(), None)), |&(prev_pointer, _)| {
                        let new_entity = #entity_type::sample_with(prev_pointer, sample_index);
                        sample_index += 1;
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
                fn sample_many_json() {
                    let entities = #entity_name::sample_many(3);
                    serde_json::to_string(&entities).expect("Failed to serialize sample entities to JSON");
                }
            }),
            bench_stream: None
        },
        FunctionDef {
            fn_stream: quote! {
                pub fn sample_with(pk: #pk_type, sample_index: usize) -> Self {
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
                pub fn sample_with_query(pk: #pk_type, sample_index: usize, stream_query: &#stream_query_type) -> Option<#entity_type> {
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
                fn sample_with_query_json() {
                    let pk = #pk_type::default();
                    let query = #stream_query_type::sample();
                    let entity = #entity_name::sample_with_query(pk, 3, &query);
                    serde_json::to_string(&entity).expect("Failed to serialize sample entity with query to JSON");
                }
            }),
            bench_stream: None
        },
    ]
}
