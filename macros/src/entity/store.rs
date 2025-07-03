use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;
use crate::rest::{FunctionDef, HttpMethod};
use crate::rest::HttpParams::FromBody;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, store_statements: &Vec<TokenStream>) -> FunctionDef {
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
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            params: vec![FromBody(entity_type.clone())],
            method: HttpMethod::POST,
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
        test_stream: Some(quote! {
                {
                    for test_entity in #entity_type::sample_many(entity_count) {
                        let pk = #entity_name::#fn_name(&db, &test_entity).expect("Failed to store and commit instance");
                        assert_eq!(test_entity.#pk_name, pk, "Stored PK does not match the instance PK");
                    }
                }
            }),
    }
}

