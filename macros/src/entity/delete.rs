use super::EntityMacros;
use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::FromPath;
use crate::rest::{FunctionDef, HttpMethod, Param};
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
        FunctionDef { entity_name: entity_name.clone(), fn_name: fn_name.clone(), fn_stream, endpoint_def: None, test_stream: None }
    }

    pub fn delete_many_def(entity_name: &Ident, pk_type: &Type, delete_many_statements: &Vec<TokenStream>) -> FunctionDef {
        let fn_name = format_ident!("delete_many");
        let fn_stream = quote! {
            pub fn #fn_name(tx: &WriteTransaction, pks: &Vec<#pk_type>) -> Result<(), AppError> {
                #(#delete_many_statements)*
                Ok(())
            }
        };
        FunctionDef { entity_name: entity_name.clone(), fn_name: fn_name.clone(), fn_stream, endpoint_def: None, test_stream: None }
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
        FunctionDef {
            entity_name: entity_name.clone(),
            fn_name: fn_name.clone(),
            fn_stream,
            endpoint_def: Some(EndpointDef {
                params: vec![FromPath(vec![Param {
                    name: pk_name.clone(),
                    ty: pk_type.clone(),
                    description: "Primary key".to_string(),
                    samples: vec![quote! { #pk_type::default().encode() }],
                }])],
                method: HttpMethod::DELETE,
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
            test_stream: Some(quote! {
                {
                    for test_entity in #entity_type::sample_many(entity_count) {
                        let pk = test_entity.#pk_name;
                        #entity_name::#fn_name(&db, &pk).expect("Failed to delete and commit instance");
                        let read_tx = db.begin_read().expect("Failed to begin read transaction");
                        let is_empty = #entity_name::get(&read_tx, &pk).expect("Failed to get instance").is_none();
                        assert!(is_empty, "Instance should be deleted");
                    }
                }
            }),
        }
    }
}
