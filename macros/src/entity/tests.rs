use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};

pub fn test_suite(entity_name: &Ident, unit_tests: Vec<TokenStream>, http_tests: Vec<TokenStream>) -> TokenStream {
    let entity_tests = format_ident!("{}_tests", entity_name.to_string().to_lowercase());
    let entity_literal = Literal::string(&entity_name.to_string());

    quote!{
        #[cfg(test)]
            mod #entity_tests {
                use super::*;
                use once_cell::sync::Lazy;
                use tokio::sync::OnceCell;

                fn test_db() -> Database {
                    Database::create(test_db_path(#entity_literal)).expect("Failed to create database")
                }

                static DB: Lazy<Arc<Database>> = Lazy::new(|| {
                    let db_path = test_db_path(#entity_literal);
                    let db = Database::create(db_path).expect("Failed to create database");
                    let entities = #entity_name::sample_many(3);
                    for entity in entities {
                        #entity_name::store_and_commit(&db, &entity).expect("Failed to persist entity");
                    }
                    Arc::new(db)
                });

                static DB_DELETE: Lazy<Arc<Database>> = Lazy::new(|| {
                    let db_path = test_db_path(#entity_literal);
                    let db = Database::create(db_path).expect("Failed to create database");
                    let entities = #entity_name::sample_many(3);
                    for entity in entities {
                        #entity_name::store_and_commit(&db, &entity).expect("Failed to persist entity");
                    }
                    Arc::new(db)
                });

                static SERVER: OnceCell<Arc<axum_test::TestServer>> = OnceCell::const_new();
                static SERVER_DELETE: OnceCell<Arc<axum_test::TestServer>> = OnceCell::const_new();

                async fn get_delete_server() -> Arc<axum_test::TestServer> {
                    SERVER_DELETE.get_or_init(|| async {
                        Arc::new(build_test_server(DB_DELETE.clone()).await)
                    }).await.clone()
                }

                async fn get_test_server() -> Arc<axum_test::TestServer> {
                    SERVER.get_or_init(|| async {
                        Arc::new(build_test_server(DB_DELETE.clone()).await)
                    }).await.clone()
                }

                #(#unit_tests)*

                #(#http_tests)*
            }
    }
}