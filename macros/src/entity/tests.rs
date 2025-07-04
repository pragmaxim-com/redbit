use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};

pub fn test_suite(entity_name: &Ident, unit_tests: Vec<TokenStream>, http_tests: Vec<TokenStream>) -> TokenStream {
    let entity_tests = format_ident!("{}_tests", entity_name.to_string().to_lowercase());
    let entity_literal = Literal::string(&entity_name.to_string());

    quote!{
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
                    #(#unit_tests)*
                }

                #[tokio::test]
                async fn test_entity_rest_api() {
                    let db = init_temp_db("rest-api");
                    let router = build_router(RequestState { db: Arc::clone(&db) }, None).await;
                    let server = axum_test::TestServer::new(router).unwrap();
                    #(#http_tests)*
                }
            }

    }
}