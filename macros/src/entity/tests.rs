use crate::field_parser::{EntityDef, OneToManyParentDef};
use crate::rest::FunctionDef;
use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};

pub fn test_suite(entity_def: &EntityDef, parent_def: Option<OneToManyParentDef>, fn_defs: &[FunctionDef]) -> TokenStream {
    let entity_name = &entity_def.entity_name;
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

    let db_init = quote! {
        fn random_storage() -> (StorageOwner, Arc<Storage>) {
            tokio::runtime::Runtime::new()
                .expect("tokio runtime")
                .block_on(StorageOwner::temp(#entity_literal, 0, true))
                .expect("Failed to create temporary storage")
        }

        fn initialize_storage(storage: Arc<Storage>) {
            let entities = #sample_entity::sample_many(#sample_count);
            let tx_context = #sample_entity::begin_write_ctx(&storage).expect("Failed to begin write transaction");
            #sample_entity::store_many(&tx_context, entities).expect("Failed to store sample entities");
            tx_context.two_phase_commit_and_close().expect("Failed to commit all");
        }
    };

    quote!{
        #[cfg(all(test, not(feature = "integration")))]
        mod #entity_tests {
            use super::*;
            use once_cell::sync::Lazy;
            use test::Bencher;
            use tokio::runtime::Runtime;

            #db_init
            
            static STORAGE: Lazy<(StorageOwner, Arc<Storage>)> = Lazy::new(|| {
                let (storage_owner, storage) = random_storage();
                initialize_storage(Arc::clone(&storage));
                (storage_owner, Arc::clone(&storage))
            });

            #(#unit_tests)*

            #(#benches)*
        }

        #[cfg(all(test, feature = "integration"))]
        mod #entity_integration_tests {
            use super::*;
            use once_cell::sync::Lazy;
            use tokio::sync::OnceCell;

            #db_init

            static SERVER: OnceCell<(StorageOwner, Arc<axum_test::TestServer>)> = OnceCell::const_new();

            async fn random_storage_async() -> (StorageOwner, Arc<Storage>) {
               StorageOwner::temp(#entity_literal, 0, true).await.expect("Failed to create temporary storage")
            }

            async fn get_delete_server() -> (StorageOwner, Arc<axum_test::TestServer>) {
                let (storage_owner, storage) = random_storage_async().await;
                initialize_storage(Arc::clone(&storage));
                let router = build_router(RequestState { storage }, None, None);
                (storage_owner, Arc::new(axum_test::TestServer::new(router).unwrap()))
            }

            async fn get_test_server() -> (&'static StorageOwner, Arc<axum_test::TestServer>) {
                let (owner, server) = SERVER.get_or_init(|| async {
                    let (storage_owner, storage) = random_storage_async().await;
                    initialize_storage(Arc::clone(&storage));
                    let router = build_router(RequestState { storage }, None, None);
                    (storage_owner, Arc::new(axum_test::TestServer::new(router).unwrap()))
                }).await;

                (owner, Arc::clone(server))
            }

            #(#http_tests)*
        }
    }
}