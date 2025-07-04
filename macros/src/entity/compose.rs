use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;
use super::EntityMacros;

impl EntityMacros {
    pub fn compose_token_streams(entity_name: &Ident, entity_type: &Type, pk_type: &Type, stream_query_ident: &Ident, field_names: &Vec<Ident>, struct_inits: &Vec<TokenStream>, struct_inits_with_query: &Vec<TokenStream>) -> Vec<TokenStream> {

        vec![
            quote! {
                fn compose(tx: &ReadTransaction, pk: &#pk_type) -> Result<#entity_type, AppError> {
                    Ok(#entity_name {
                        #(#struct_inits),*
                    })
                }
            },
            quote! {
                fn compose_with_filter(tx: &ReadTransaction, pk: &#pk_type, streaming_query: #stream_query_ident) -> Result<Option<#entity_type>, AppError> {
                    // First: fetch & filter every column, short‑circuit on mismatch
                    #(#struct_inits_with_query)*
                    Ok(Some(#entity_type {
                        #(#field_names,)*
                    }))
                }
            },
        ]
    }

}
