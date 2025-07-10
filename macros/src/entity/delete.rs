use super::EntityMacros;
use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::FromPath;
use crate::rest::{FunctionDef, HttpMethod, PathExpr};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;

impl EntityMacros {
    pub fn delete_def(entity_name: &Ident, pk_type: &Type, delete_statements: &Vec<TokenStream>) -> FunctionDef {
        let fn_name = format_ident!("delete");
        let fn_stream = quote! {
            pub fn #fn_name(tx: &WriteTransaction, pk: &#pk_type) -> Result<(), AppError> {
                #(#delete_statements)*
                Ok(())
            }
        };
        FunctionDef { entity_name: entity_name.clone(), fn_name: fn_name.clone(), fn_stream, endpoint_def: None, test_stream: None, bench_stream: None }
    }

    pub fn delete_many_def(entity_name: &Ident, pk_type: &Type, delete_many_statements: &Vec<TokenStream>) -> FunctionDef {
        let fn_name = format_ident!("delete_many");
        let fn_stream = quote! {
            pub fn #fn_name(tx: &WriteTransaction, pks: &Vec<#pk_type>) -> Result<(), AppError> {
                #(#delete_many_statements)*
                Ok(())
            }
        };
        FunctionDef { entity_name: entity_name.clone(), fn_name: fn_name.clone(), fn_stream, endpoint_def: None, test_stream: None, bench_stream: None }
    }

    pub fn delete_and_commit_def(
        entity_name: &Ident,
        entity_type: &Type,
        pk_name: &Ident,
        pk_type: &Type,
        delete_statements: &Vec<TokenStream>,
    ) -> FunctionDef {
        let fn_name = format_ident!("delete_and_commit");
        let fn_stream = quote! {
            pub fn #fn_name(db: &Database, pk: &#pk_type) -> Result<(), AppError> {
               let tx = db.begin_write()?;
               {
                   #(#delete_statements)*
               }
               tx.commit()?;
               Ok(())
           }
        };

        let test_stream = Some(quote! {
            #[test]
            fn #fn_name() {
                let db = test_db();
                let entity_count: usize = 3;
                for test_entity in #entity_type::sample_many(entity_count) {
                    #entity_name::store_and_commit(&db, &test_entity).expect("Failed to store and commit instance");
                    let pk = test_entity.#pk_name;
                    #entity_name::#fn_name(&db, &pk).expect("Failed to delete and commit instance");
                    let read_tx = db.begin_read().expect("Failed to begin read transaction");
                    let is_empty = #entity_name::get(&read_tx, &pk).expect("Failed to get instance").is_none();
                    assert!(is_empty, "Instance should be deleted");
                }
            }
        });

        let bench_fn_name = format_ident!("_{}", fn_name);
        let bench_stream = Some(quote! {
            #[bench]
            fn #bench_fn_name(b: &mut Bencher) {
                let db = test_db();
                let test_entity = #entity_type::sample();
                #entity_name::store_and_commit(&db, &test_entity).expect("Failed to store and commit instance");
                let pk = test_entity.#pk_name;
                b.iter(|| {
                    #entity_name::#fn_name(&db, &pk).expect("Failed to delete and commit instance");
                });
            }
        });

        let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

        FunctionDef {
            entity_name: entity_name.clone(),
            fn_name: fn_name.clone(),
            fn_stream,
            endpoint_def: Some(EndpointDef {
                params: vec![FromPath(vec![PathExpr {
                    name: pk_name.clone(),
                    ty: pk_type.clone(),
                    description: "Primary key".to_string(),
                    sample: quote! { #pk_type::default().encode() },
                }])],
                method: HttpMethod::DELETE,
                handler_name: format_ident!("{}", handler_fn_name),
                utoipa_responses: quote! { responses((status = OK)) },
                handler_impl_stream: quote! {
                    Result<AppJson<()>, AppError> {
                        let db = state.db;
                        let result = #entity_name::#fn_name(&db, &#pk_name)?;
                        Ok(AppJson(result))
                    }
                },
                endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), pk_name, pk_name),
            }),
            test_stream,
            bench_stream
        }
    }
}
