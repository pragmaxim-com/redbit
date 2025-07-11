use super::EntityMacros;
use proc_macro2::TokenStream;
use quote::quote;

impl EntityMacros {
    pub fn expand(&self) -> TokenStream {
        let EntityMacros {
            stream_query_struct,
            range_query_structs,
            api_functions,
            endpoint_handlers,
            routes,
            sample_functions,
            compose_functions,
            entity_name,
            table_definitions,
            test_suite,
            client_calls,
        } = self;

        quote! {
            // StreamQuery is passed from the rest api as POST body and used to filter the stream of entities
            #stream_query_struct
            // Query structs to map query params into
            #(#range_query_structs)*
            // table definitions are not in the impl object because they are accessed globally with semantic meaning
            #(#table_definitions)*
            // axum endpoints cannot be in the impl object https://docs.rs/axum/latest/axum/attr.debug_handler.html#limitations
            #(#endpoint_handlers)*

            impl #entity_name {
                // api functions are exposed to users
                #(#api_functions)*
                // sample functions are used to generate test data
                #(#sample_functions)*
                // compose functions build entities from db results
                #(#compose_functions)*
                // axum routes
                #routes
                // client calls are executed from node.js runtime
                #client_calls
            }
            // unit tests and rest api tests
            #test_suite
        }.into()
    }
}
