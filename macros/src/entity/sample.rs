use crate::entity::EntityMacros;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

impl EntityMacros {
    pub fn sample_token_streams(entity_name: &Ident, entity_type: &Type, pk_type: &Type, struct_default_inits: &Vec<TokenStream>) -> Vec<TokenStream> {
        vec![
            quote! {
                pub fn sample() -> Self {
                    #entity_name::sample_with(&#pk_type::default(), 0)
                }
            },
            quote! {
                pub fn sample_many(n: usize) -> Vec<#entity_type> {
                    let mut sample_index = 0;
                    std::iter::successors(Some((#pk_type::default(), None)), |(prev_pointer, _)| {
                        let new_entity = #entity_type::sample_with(prev_pointer, sample_index);
                        sample_index += 1;
                        Some((prev_pointer.next(), Some(new_entity)))
                    })
                    .filter_map(|(_, instance)| instance)
                    .take(n)
                    .collect()
                }
            },
            quote! {
                pub fn sample_with(pk: &#pk_type, sample_index: usize) -> Self {
                    #entity_name {
                        #(#struct_default_inits),*
                    }
                }
            },
        ]
    }
}
