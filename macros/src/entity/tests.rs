use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use crate::rest::FunctionDef;

pub fn test_suite(entity_name: &Ident, parent_entity: Option<Ident>, fn_defs: &Vec<FunctionDef>) -> TokenStream {
    let entity_tests = format_ident!("{}", entity_name.to_string().to_lowercase());
    let entity_literal = Literal::string(&entity_name.to_string());
    let http_tests = fn_defs.iter().filter_map(|f| f.endpoint.clone().map(|e| e.tests)).flatten().collect::<Vec<_>>();
    let unit_tests = fn_defs.iter().filter_map(|f| f.test_stream.clone()).collect::<Vec<_>>();
    let benches = fn_defs.iter().filter_map(|f| f.bench_stream.clone()).collect::<Vec<_>>();

    let (sample_count, sample_entity) =
        match parent_entity {
            Some(parent) => (1usize, parent),
            None => (3usize, entity_name.clone()),
        };

    quote!{
        #[cfg(test)]
            mod #entity_tests {
                use super::*;
                use once_cell::sync::Lazy;
                use tokio::sync::OnceCell;
                use test::Bencher;
                use tokio::runtime::Runtime;

                fn test_db() -> Database {
                    Database::create(test_db_path(#entity_literal)).expect("Failed to create database")
                }

                static DB: Lazy<Arc<Database>> = Lazy::new(|| {
                    let db = test_db();
                    let entities = #sample_entity::sample_many(#sample_count);
                    for entity in entities {
                        #sample_entity::store_and_commit(&db, &entity).expect("Failed to persist entity");
                    }
                    Arc::new(db)
                });

                static DB_DELETE: Lazy<Arc<Database>> = Lazy::new(|| {
                    let db = test_db();
                    let entities = #sample_entity::sample_many(#sample_count);
                    for entity in entities {
                        #sample_entity::store_and_commit(&db, &entity).expect("Failed to persist entity");
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

                #(#benches)*

            }
    }
}