use super::EntityMacros;
use crate::macro_utils;
use crate::rest::to_http_endpoints;
use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};

impl EntityMacros {
    pub fn expand(&self) -> TokenStream {
        let stream_query_struct = &self.stream_query_struct;
        let range_query_structs = &self.range_query_structs;
        let function_defs = &self.function_defs;
        let sample_functions = &self.sample_functions;
        let compose_functions = &self.compose_functions;
        let entity_name = &self.entity_name;
        let entity_tests = format_ident!("{}_tests", &self.entity_name.to_string().to_lowercase());
        let entity_literal = Literal::string(&self.entity_name.to_string());
        
        let functions: Vec<TokenStream> = function_defs.iter().map(|f| f.fn_stream.clone()).collect::<Vec<_>>();
        let table_definitions: Vec<TokenStream> = self.table_definitions.iter().map(|table_def| table_def.definition.clone()).collect();
        let tests = function_defs.iter().filter_map(|f| f.test_stream.clone()).collect::<Vec<_>>();
        let (endpoint_handlers, routes, route_tests) = to_http_endpoints(&function_defs);
        
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

            #[cfg(test)]
            mod #entity_tests {
                use super::*;

                fn init_temp_db(name: &str) -> Arc<Database> {
                    let dir = std::env::temp_dir().join("redbit").join(name).join(#entity_literal);
                    if !dir.exists() {
                        std::fs::create_dir_all(dir.clone()).unwrap();
                    }
                    let db_path = dir.join(format!("{}_{}.redb", #entity_literal, rand::random::<u64>()));
                    Arc::new(Database::create(db_path).expect("Failed to create database"))
                }

                #[tokio::test]
                async fn test_entity_api() {
                    let db = init_temp_db("api");
                    let entity_count: usize = 3;
                    #(#tests)*
                }

                #[tokio::test]
                async fn test_entity_rest_api() {
                    let db = init_temp_db("rest-api");
                    let router = build_router(RequestState { db: Arc::clone(&db) }, None).await;
                    let server = axum_test::TestServer::new(router).unwrap();
                    #(#route_tests)*
                }
            }
        };
        // eprintln!("----------------------------------------------------------");
        macro_utils::write_stream_and_return(expanded, &entity_name)
    }

}
