use crate::endpoint::EndpointDef;
use crate::field_parser::EntityDef;
use crate::rest::HttpParams::Body;
use crate::rest::{BodyExpr, EndpointTag, FunctionDef, HttpMethod};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use crate::relationship::{StoreStatement, WriteFromStatement};

pub fn store_def(entity_def: &EntityDef, mixed_statements: &[StoreStatement]) -> FunctionDef {
    let entity_name = &entity_def.entity_name;
    let entity_type = &entity_def.entity_type;
    let write_ctx_type = &entity_def.write_ctx_type;
    let fn_name = format_ident!("store");

    let mut store_stmts: Vec<TokenStream> = Vec::new();
    let mut write_from_stmts: Vec<WriteFromStatement> = Vec::new();

    for stmt in mixed_statements {
        match stmt {
            StoreStatement::Plain(ts) => store_stmts.push(ts.clone()),
            StoreStatement::WriteFrom { single, .. } => write_from_stmts.push(single.clone()),
        }
    }

    // Prepare the pieces derived from write-from hooks
    let write_from_inits: Vec<TokenStream> = write_from_stmts.iter().map(|wfs| wfs.init.clone()).collect();
    let write_from_collects: Vec<TokenStream> = write_from_stmts.iter().map(|wfs| wfs.collect.clone()).collect();
    let write_from_stores: Vec<TokenStream> = write_from_stmts.iter().map(|wfs| wfs.store.clone()).collect();

    let fn_stream = quote! {
        fn #fn_name(tx_context: &#write_ctx_type, instance: #entity_type) -> Result<(), AppError> {
            let is_last = true;
            #(#store_stmts)*
            #(#write_from_inits)*
            #(#write_from_collects)*
            #(#write_from_stores)*
            Ok(())
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = random_storage();
            let entity_count: usize = 3;
            for test_entity in #entity_type::sample_many(Default::default(), entity_count) {
                let ctx = #entity_name::begin_write_ctx(&storage, Durability::None).unwrap();
                ctx.two_phase_commit_or_rollback_and_close_with(|tx_context| {
                    let _ = #entity_name::#fn_name(&tx_context, test_entity)?;
                    Ok(())
                }).expect("Failed to store and commit instance");
            }
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = random_storage();
            let test_entity = #entity_type::sample();
            let tx_context = #entity_name::new_write_ctx(&storage).unwrap();
            b.iter(|| {
                let _ = tx_context.begin_writing(Durability::None).expect("Failed to begin writing");
                #entity_name::#fn_name(&tx_context, test_entity.clone()).expect("Failed to store and commit instance");
                for f in tx_context.commit_ctx_async().unwrap() {
                    f.wait().expect("Failed to commit");
                }
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

pub fn store_many_def(entity_def: &EntityDef, mixed_statements: &[StoreStatement]) -> FunctionDef {
    let entity_name = &entity_def.entity_name;
    let entity_type = &entity_def.entity_type;
    let write_ctx_type = &entity_def.write_ctx_type;
    let fn_name = format_ident!("store_many");

    let mut store_stmts: Vec<TokenStream> = Vec::new();
    let mut write_from_stmts: Vec<WriteFromStatement> = Vec::new();

    for stmt in mixed_statements {
        match stmt {
            StoreStatement::Plain(ts) => store_stmts.push(ts.clone()),
            StoreStatement::WriteFrom { multi, .. } => write_from_stmts.push(multi.clone()),
        }
    }

    // Prepare the pieces derived from write-from hooks
    let write_from_inits: Vec<TokenStream> = write_from_stmts.iter().map(|wfs| wfs.init.clone()).collect();
    let write_from_collects: Vec<TokenStream> = write_from_stmts.iter().map(|wfs| wfs.collect.clone()).collect();
    let write_from_stores: Vec<TokenStream> = write_from_stmts.iter().map(|wfs| wfs.store.clone()).collect();

    // store_stmts will be expanded in the loop body
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#write_ctx_type, instances: Vec<#entity_type>, is_last: bool) -> Result<(), AppError> {
            let mut remaining = instances.len();
            #(#write_from_inits)*
            for instance in instances {
                remaining -= 1;
                let is_last = is_last && remaining == 0;
                #(#store_stmts)*
                #(#write_from_collects)*
            }
            #(#write_from_stores)*
            Ok(())
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = random_storage();
            let entity_count: usize = 3;
            let test_entities = #entity_type::sample_many(Default::default(), entity_count);
            let ctx = #entity_name::begin_write_ctx(&storage, Durability::None).unwrap();
            ctx.two_phase_commit_or_rollback_and_close_with(|tx_context| {
                let _ = #entity_name::#fn_name(&tx_context, test_entities, true)?;
                Ok(())
            }).expect("Failed to store and commit instances");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = random_storage();
            let entity_count = 3;
            let test_entities = #entity_type::sample_many(Default::default(), entity_count);
            let ctx = #entity_name::new_write_ctx(&storage).unwrap();
            b.iter(|| {
                let _ = ctx.begin_writing(Durability::None).expect("Failed to begin writing");
                ctx.two_phase_commit_with(|tx_context| {
                    #entity_name::#fn_name(&tx_context, test_entities.clone(), true)?;
                    Ok(())
                })?;
            });
            ctx.stop_writing().unwrap();
        }
    });

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream,
        bench_stream
    }
}

pub fn persist_def(entity_def: &EntityDef, mixed_statements: &[StoreStatement]) -> FunctionDef {
    let fn_name = format_ident!("persist");
    let entity_name = &entity_def.entity_name;
    let entity_type = &entity_def.entity_type;
    let key_def = &entity_def.key_def.field_def();
    let pk_name = &key_def.name;
    let pk_type = &key_def.tpe;

    let mut store_stmts: Vec<TokenStream> = Vec::new();
    let mut write_from_stmts: Vec<WriteFromStatement> = Vec::new();

    for stmt in mixed_statements {
        match stmt {
            StoreStatement::Plain(ts) => store_stmts.push(ts.clone()),
            StoreStatement::WriteFrom { single, .. } => write_from_stmts.push(single.clone()),
        }
    }

    let write_from_inits: Vec<TokenStream> = write_from_stmts.iter().map(|wfs| wfs.init.clone()).collect();
    let write_from_collects: Vec<TokenStream> = write_from_stmts.iter().map(|wfs| wfs.collect.clone()).collect();
    let write_from_stores: Vec<TokenStream> = write_from_stmts.iter().map(|wfs| wfs.store.clone()).collect();

    let fn_stream = quote! {
        pub fn #fn_name(storage: Arc<Storage>, instance: #entity_type) -> Result<#pk_type, AppError> {
           let pk = instance.#pk_name;
           let is_last = true;
           let ctx = #entity_name::begin_write_ctx(&storage, Durability::Immediate)?;
           ctx.two_phase_commit_or_rollback_and_close_with(|tx_context| {
               #(#store_stmts)*
               #(#write_from_inits)*
               #(#write_from_collects)*
               #(#write_from_stores)*
               Ok(())
           })?;
           Ok(pk)
       }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = random_storage();
            let entity_count: usize = 3;
            for test_entity in #entity_type::sample_many(Default::default(), entity_count) {
                let pk = #entity_name::#fn_name(Arc::clone(&storage), test_entity.clone()).expect("Failed to store and commit instance");
                assert_eq!(test_entity.#pk_name, pk, "Stored PK does not match the instance PK");
            }
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = random_storage();
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
