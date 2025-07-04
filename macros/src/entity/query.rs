use super::EntityMacros;
use proc_macro2::{Ident, TokenStream};
use quote::quote;

impl EntityMacros {
    pub fn query_struct_token_stream(stream_query_ident: &Ident, stream_queries: &Vec<(TokenStream, TokenStream)>) -> TokenStream {
        let definitions: Vec<TokenStream> = stream_queries.iter().map(|(def, _)| def.clone()).collect();
        let inits: Vec<TokenStream> = stream_queries.iter().map(|(_, init)| init.clone()).collect();
        quote! {
            #[derive(IntoParams, Serialize, Deserialize, Default)]
            pub struct #stream_query_ident {
                #(#definitions),*
            }
            impl #stream_query_ident {
                pub fn sample() -> Self {
                    Self {
                        #(#inits),*
                    }
                }
            }
        }
    }
}
