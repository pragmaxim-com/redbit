use super::EntityMacros;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Type;

impl EntityMacros {
    pub fn query_struct_token_stream(stream_query_type: &Type, stream_queries: &Vec<(TokenStream, TokenStream)>) -> TokenStream {
        let definitions: Vec<TokenStream> = stream_queries.iter().map(|(def, _)| def.clone()).collect();
        let inits: Vec<TokenStream> = stream_queries.iter().map(|(_, init)| init.clone()).collect();
        quote! {
            #[derive(Clone, Debug, IntoParams, Serialize, Deserialize, Default, ToSchema)]
            pub struct #stream_query_type {
                #(#definitions),*
            }
            impl #stream_query_type {
                pub fn sample() -> Self {
                    Self {
                        #(#inits),*
                    }
                }
            }
        }
    }
}
