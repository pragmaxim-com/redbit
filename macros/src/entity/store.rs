use proc_macro2::{Ident, TokenStream};
use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::Body;
use crate::rest::{BodyExpr, EndpointTag, FunctionDef, HttpMethod};
use quote::{format_ident, quote};
use syn::Type;
use crate::table::StoreManyStmnt;

pub fn store_def(entity_name: &Ident, entity_type: &Type, store_statements: &[TokenStream]) -> FunctionDef {
    let fn_name = format_ident!("store");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &StorageWriteTx, instance: #entity_type) -> Result<(), AppError> {
            #(#store_statements)*
            Ok(())
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = random_storage();
            let entity_count: usize = 3;
            for test_entity in #entity_type::sample_many(entity_count) {
                let tx = storage.begin_write().expect("Failed to begin write transaction");
                let pk = #entity_name::#fn_name(&tx, test_entity).expect("Failed to store and commit instance");
                tx.commit().expect("Failed to commit transaction");
            }
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = random_storage();
            let test_entity = #entity_type::sample();
            b.iter(|| {
                let tx = storage.begin_write().expect("Failed to begin write transaction");
                #entity_name::#fn_name(&tx, test_entity.clone()).expect("Failed to store and commit instance");
                tx.commit().expect("Failed to commit transaction");
            });
        }
    });

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream,
        bench_stream
    }
}

pub fn store_many_def(entity_name: &Ident, entity_type: &Type, store_many_statements: &[StoreManyStmnt]) -> FunctionDef {
    let fn_name = format_ident!("store_many");
    let pres: Vec<TokenStream> = store_many_statements.iter().map(|s| s.pre.clone()).collect();
    let inserts: Vec<TokenStream> = store_many_statements.iter().map(|s| s.insert.clone()).collect();
    let posts: Vec<TokenStream> = store_many_statements.iter().map(|s| s.post.clone()).collect();
    let fn_stream = quote! {
        pub fn #fn_name(tx: &StorageWriteTx, instances: Vec<#entity_type>) -> Result<(), AppError> {
            #(#pres)*
            for mut instance in instances {
                #(#inserts)*
            }
            #(#posts)*
            Ok(())
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = random_storage();
            let entity_count: usize = 3;
            let test_entities = #entity_type::sample_many(entity_count);
            let tx = storage.begin_write().expect("Failed to begin write transaction");
            let pk = #entity_name::#fn_name(&tx, test_entities).expect("Failed to store and commit instance");
            tx.commit().expect("Failed to commit transaction");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = random_storage();
            let entity_count = 3;
            let test_entities = #entity_type::sample_many(entity_count);
            b.iter(|| {
                let tx = storage.begin_write().expect("Failed to begin write transaction");
                #entity_name::#fn_name(&tx, test_entities.clone()).expect("Failed to store and commit instance");
                tx.commit().expect("Failed to commit transaction");
            });
        }
    });


    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream,
        bench_stream
    }
}

pub fn store_and_commit_def(entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, store_statements: &[TokenStream]) -> FunctionDef {
    let fn_name = format_ident!("store_and_commit");
    let fn_stream = quote! {
        pub fn #fn_name(storage: Arc<Storage>, instance: #entity_type) -> Result<#pk_type, AppError> {
           let tx = storage.begin_write()?;
           let pk = instance.#pk_name;
           {
               #(#store_statements)*
           }
           tx.commit()?;
           Ok(pk)
       }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = random_storage();
            let entity_count: usize = 3;
            for test_entity in #entity_type::sample_many(entity_count) {
                let pk = #entity_name::#fn_name(Arc::clone(&storage), test_entity.clone()).expect("Failed to store and commit instance");
                assert_eq!(test_entity.#pk_name, pk, "Stored PK does not match the instance PK");
            }
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = random_storage();
            let test_entity = #entity_type::sample();
            b.iter(|| {
                #entity_name::#fn_name(Arc::clone(&storage), test_entity.clone()).expect("Failed to store and commit instance");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            _entity_name: entity_name.clone(),
            tag: EndpointTag::DataWrite,
            fn_name: fn_name.clone(),
            params: vec![Body(BodyExpr {
                ty: entity_type.clone(),
                extraction: quote! { AppJson(body): AppJson<#entity_type> },
                samples: quote! { vec![#entity_type::sample()] },
                required: true,
            })],
            method: HttpMethod::POST,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
                impl IntoResponse {
                    match #entity_name::#fn_name(Arc::clone(&state.storage), body) {
                        Ok(_) => Response::builder().status(StatusCode::OK).body(Body::empty()).unwrap().into_response(),
                        Err(err) => err.into_response(),
                    }
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}", entity_name.to_string().to_lowercase()),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}
