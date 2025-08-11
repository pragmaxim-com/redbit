use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

pub fn compose_token_stream(entity_name: &Ident, entity_type: &Type, pk_type: &Type, struct_inits: &Vec<TokenStream>) -> TokenStream {
    quote! {
        fn compose(tx: &StorageReadTx, pk: &#pk_type) -> Result<#entity_type, AppError> {
            Ok(#entity_name {
                #(#struct_inits),*
            })
        }
    }
}
pub fn compose_with_filter_token_stream(entity_type: &Type, pk_type: &Type, stream_query_type: &Type, field_names: &Vec<Ident>, struct_inits_with_query: &Vec<TokenStream>) -> TokenStream {
    quote! {
        fn compose_with_filter(tx: &StorageReadTx, pk: &#pk_type, stream_query: &#stream_query_type) -> Result<Option<#entity_type>, AppError> {
            // First: fetch & filter every column, short‑circuit on mismatch
            #(#struct_inits_with_query)*
            Ok(Some(#entity_type {
                #(#field_names,)*
            }))
        }
    }
}
