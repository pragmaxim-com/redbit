use proc_macro2::{Ident, TokenStream};
use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::Body;
use crate::rest::{BodyExpr, EndpointTag, FunctionDef, HttpMethod};
use quote::{format_ident, quote};
use syn::Type;

pub fn store_def(entity_name: &Ident, entity_type: &Type, tx_context_ty: &Type, store_statements: &[TokenStream]) -> FunctionDef {
    let fn_name = format_ident!("store");
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#tx_context_ty, instance: #entity_type) -> Result<(), AppError> {
            #(#store_statements)*
            Ok(())
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = random_storage();
            let entity_count: usize = 3;
            let tx_context = #entity_name::begin_write_ctx(&storage).unwrap();
            for test_entity in #entity_type::sample_many(entity_count) {
                let pk = #entity_name::#fn_name(&tx_context, test_entity).expect("Failed to store and commit instance");
            }
            tx_context.commit_and_close_ctx().expect("Failed to flush transaction context");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = random_storage();
            let test_entity = #entity_type::sample();
            let tx_context = #entity_name::new_write_ctx(&storage).unwrap();
            b.iter(|| {
                let _ = tx_context.begin_writing().expect("Failed to begin writing");
                #entity_name::#fn_name(&tx_context, test_entity.clone()).expect("Failed to store and commit instance");
                let _ = tx_context.two_phase_commit().expect("Failed to commit");
            });
            tx_context.stop_writing().unwrap();
        }
    });

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream,
        bench_stream
    }
}

pub fn store_many_def(entity_name: &Ident, entity_type: &Type, tx_context_ty: &Type, store_many_statements: &[TokenStream]) -> FunctionDef {
    let fn_name = format_ident!("store_many");
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#tx_context_ty, instances: Vec<#entity_type>) -> Result<(), AppError> {
            for instance in instances {
                #(#store_many_statements)*
            }
            Ok(())
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = random_storage();
            let entity_count: usize = 3;
            let test_entities = #entity_type::sample_many(entity_count);
            let tx_context = #entity_name::begin_write_ctx(&storage).unwrap();
            let pk = #entity_name::#fn_name(&tx_context, test_entities).expect("Failed to store and commit instance");
            tx_context.commit_and_close_ctx().expect("Failed to flush transaction context");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = random_storage();
            let entity_count = 3;
            let test_entities = #entity_type::sample_many(entity_count);
            let tx_context = #entity_name::new_write_ctx(&storage).unwrap();
            b.iter(|| {
                let _ = tx_context.begin_writing().expect("Failed to begin writing");
                #entity_name::#fn_name(&tx_context, test_entities.clone()).expect("Failed to store and commit instance");
                let _ = tx_context.two_phase_commit().expect("Failed to commit");
            });
            tx_context.stop_writing().unwrap();
        }
    });


    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream,
        bench_stream
    }
}

pub fn persist_def(entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, store_statements: &[TokenStream]) -> FunctionDef {
    let fn_name = format_ident!("persist");
    let fn_stream = quote! {
        pub fn #fn_name(storage: Arc<Storage>, instance: #entity_type) -> Result<#pk_type, AppError> {
           let pk = instance.#pk_name;
           let tx_context = #entity_name::begin_write_ctx(&storage)?;
           #(#store_statements)*
           tx_context.two_phase_commit_and_close()?;
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
            return_type: None,
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
