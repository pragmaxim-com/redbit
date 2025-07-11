use proc_macro2::{Ident, TokenStream};
use super::EntityMacros;
use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::FromBody;
use crate::rest::{FunctionDef, HttpMethod, Param};
use quote::{format_ident, quote};
use syn::Type;

impl EntityMacros {
    pub fn store_def(entity_name: &Ident, entity_type: &Type, store_statements: &Vec<TokenStream>) -> FunctionDef {
        let fn_name = format_ident!("store");
        let fn_stream = quote! {
            pub fn #fn_name(tx: &WriteTransaction, instance: &#entity_type) -> Result<(), AppError> {
                #(#store_statements)*
                Ok(())
            }
        };

        let test_stream = Some(quote! {
            #[test]
            fn #fn_name() {
                let db = test_db();
                let entity_count: usize = 3;
                for test_entity in #entity_type::sample_many(entity_count) {
                    let tx = db.begin_write().expect("Failed to begin write transaction");
                    let pk = #entity_name::#fn_name(&tx, &test_entity).expect("Failed to store and commit instance");
                    tx.commit().expect("Failed to commit transaction");
                }
            }
        });

        let bench_fn_name = format_ident!("_{}", fn_name);
        let bench_stream = Some(quote! {
            #[bench]
            fn #bench_fn_name(b: &mut Bencher) {
                let db = test_db();
                let test_entity = #entity_type::sample();
                b.iter(|| {
                    let tx = db.begin_write().expect("Failed to begin write transaction");
                    #entity_name::#fn_name(&tx, &test_entity).expect("Failed to store and commit instance");
                    tx.commit().expect("Failed to commit transaction");
                });
            }
        });

        FunctionDef {
            entity_name: entity_name.clone(),
            fn_name: fn_name.clone(),
            fn_stream,
            endpoint_def: None,
            test_stream,
            bench_stream
        }
    }

    pub fn store_many_def(entity_name: &Ident, entity_type: &Type, store_many_statements: &Vec<TokenStream>) -> FunctionDef {
        let fn_name = format_ident!("store_many");
        let fn_stream = quote! {
            pub fn #fn_name(tx: &WriteTransaction, instances: &Vec<#entity_type>) -> Result<(), AppError> {
                #(#store_many_statements)*
                Ok(())
            }
        };

        let test_stream = Some(quote! {
            #[test]
            fn #fn_name() {
                let db = test_db();
                let entity_count: usize = 3;
                let test_entities = #entity_type::sample_many(entity_count);
                let tx = db.begin_write().expect("Failed to begin write transaction");
                let pk = #entity_name::#fn_name(&tx, &test_entities).expect("Failed to store and commit instance");
                tx.commit().expect("Failed to commit transaction");
            }
        });

        let bench_fn_name = format_ident!("_{}", fn_name);
        let bench_stream = Some(quote! {
            #[bench]
            fn #bench_fn_name(b: &mut Bencher) {
                let db = test_db();
                let entity_count = 3;
                let test_entities = #entity_type::sample_many(entity_count);
                b.iter(|| {
                    let tx = db.begin_write().expect("Failed to begin write transaction");
                    #entity_name::#fn_name(&tx, &test_entities).expect("Failed to store and commit instance");
                    tx.commit().expect("Failed to commit transaction");
                });
            }
        });


        FunctionDef {
            entity_name: entity_name.clone(),
            fn_name: fn_name.clone(),
            fn_stream,
            endpoint_def: None,
            test_stream,
            bench_stream
        }
    }

    pub fn store_and_commit_def(entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, store_statements: &Vec<TokenStream>) -> FunctionDef {
        let fn_name = format_ident!("store_and_commit");
        let fn_stream = quote! {
            pub fn #fn_name(db: &Database, instance: &#entity_type) -> Result<#pk_type, AppError> {
               let tx = db.begin_write()?;
               {
                   #(#store_statements)*
               }
               tx.commit()?;
               Ok(instance.#pk_name.clone())
           }
        };

        let test_stream = Some(quote! {
            #[test]
            fn #fn_name() {
                let db = test_db();
                let entity_count: usize = 3;
                for test_entity in #entity_type::sample_many(entity_count) {
                    let pk = #entity_name::#fn_name(&db, &test_entity).expect("Failed to store and commit instance");
                    assert_eq!(test_entity.#pk_name, pk, "Stored PK does not match the instance PK");
                }
            }
        });

        let bench_fn_name = format_ident!("_{}", fn_name);
        let bench_stream = Some(quote! {
            #[bench]
            fn #bench_fn_name(b: &mut Bencher) {
                let db = test_db();
                let test_entity = #entity_type::sample();
                b.iter(|| {
                    #entity_name::#fn_name(&db, &test_entity).expect("Failed to store and commit instance");
                });
            }
        });

        let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

        FunctionDef {
            entity_name: entity_name.clone(),
            fn_name: fn_name.clone(),
            fn_stream,
            endpoint_def: Some(EndpointDef {
                params: vec![FromBody(Param {
                    name: format_ident!("body"), // TODO 
                    ty: entity_type.clone(),
                    description: "Entity instance to store".to_string(),
                    samples: quote! { vec![#entity_type::sample()] },
                })],
                method: HttpMethod::POST,
                handler_name: format_ident!("{}", handler_fn_name),
                client_call: None,
                handler_impl_stream: quote! {
                    Result<AppJson<#pk_type>, AppError> {
                        let db = state.db;
                        let result = #entity_name::#fn_name(&db, &body)?;
                        Ok(AppJson(result))
                    }
                },
                utoipa_responses: quote! { responses((status = OK, body = #pk_type)) },
                endpoint: format!("/{}", entity_name.to_string().to_lowercase()),
            }),
            test_stream,
            bench_stream
        }
    }
}
