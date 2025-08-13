use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use crate::field_parser::ParentDef;
use crate::rest::FunctionDef;

pub fn test_suite(entity_name: &Ident, parent_def: Option<ParentDef>, fn_defs: &[FunctionDef]) -> TokenStream {
    let parent_entity = parent_def.clone().map(|p|p.parent_ident);
    let entity_tests = format_ident!("{}", entity_name.to_string().to_lowercase());
    let entity_integration_tests = format_ident!("{}_integration", entity_name.to_string().to_lowercase());
    let entity_literal = Literal::string(&entity_name.to_string());
    let http_tests = fn_defs.iter().filter_map(|f| f.endpoint.clone().map(|e| e.tests)).flatten().collect::<Vec<_>>();
    let unit_tests = fn_defs.iter().filter_map(|f| f.test_stream.clone()).collect::<Vec<_>>();
    let benches = fn_defs.iter().filter_map(|f| f.bench_stream.clone()).collect::<Vec<_>>();

    let (sample_count, sample_entity) =
        match parent_entity {
            Some(parent) => (1usize, parent),
            None => (3usize, entity_name.clone()),
        };

    let db_init = quote!{
        fn random_storage() -> Arc<Storage> {
            create_random_storage(#entity_literal)
        }

        static STORAGE: Lazy<Arc<Storage>> = Lazy::new(|| {
            let storage = random_storage();
            let entities = #sample_entity::sample_many(#sample_count);
            for entity in entities {
                #sample_entity::store_and_commit(Arc::clone(&storage), &entity).expect("Failed to persist entity");
            }
            Arc::clone(&storage)
        });
    };

    quote!{
        #[cfg(all(test, not(feature = "integration")))]
        mod #entity_tests {
            use super::*;
            use once_cell::sync::Lazy;
            use tokio::sync::OnceCell;
            use test::Bencher;
            use tokio::runtime::Runtime;

            #db_init

            #(#unit_tests)*

            #(#benches)*
        }

        #[cfg(all(test, feature = "integration"))]
        mod #entity_integration_tests {
            use super::*;
            use once_cell::sync::Lazy;
            use tokio::sync::OnceCell;

            #db_init

            static SERVER: OnceCell<Arc<axum_test::TestServer>> = OnceCell::const_new();

            async fn get_test_server() -> Arc<axum_test::TestServer> {
                SERVER.get_or_init(|| async {
                    let storage = STORAGE.clone();
                    let router = build_router(RequestState { storage }, None, None).await;
                    Arc::new(axum_test::TestServer::new(router).unwrap())
                }).await.clone()
            }

            #(#http_tests)*
        }
    }
}