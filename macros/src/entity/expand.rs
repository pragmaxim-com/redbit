use super::EntityMacros;
use crate::macro_utils;
use proc_macro2::TokenStream;
use quote::quote;

impl EntityMacros {
    pub fn expand(&self) -> TokenStream {
        let EntityMacros {
            stream_query_struct,
            range_query_structs,
            functions,
            endpoint_handlers,
            routes,
            sample_functions,
            compose_functions,
            entity_name,
            table_definitions,
            test_suite,
        } = self;
        
        let expanded = quote! {
            #stream_query_struct
            // table definitions are not in the impl object because they are accessed globally with semantic meaning
            #(#table_definitions)*
            // utoipa and axum query structs to map query and path params into
            #(#range_query_structs)*
            // axum endpoints cannot be in the impl object https://docs.rs/axum/latest/axum/attr.debug_handler.html#limitations
            #(#endpoint_handlers)*

            impl #entity_name {
                #(#functions)*
                #(#sample_functions)*
                #(#compose_functions)*
                #routes
            }

            #test_suite
        };
        // eprintln!("----------------------------------------------------------");
        macro_utils::write_stream_and_return(expanded, &entity_name)
    }
}
