use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

pub fn stream_query_struct_macro(entity_name: &Ident, stream_queries: &Vec<(TokenStream, TokenStream)>) -> (Ident, TokenStream) {
    let stream_query_ident = format_ident!("{}StreamQuery", entity_name.to_string());
    let definitions: Vec<TokenStream> = stream_queries.iter().map(|(def, _)| def.clone()).collect();
    let inits: Vec<TokenStream> = stream_queries.iter().map(|(_, init)| init.clone()).collect();
    let token_stream =
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
        };
    (stream_query_ident, token_stream)
}
