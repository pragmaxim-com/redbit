use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::FromPath;
use crate::rest::{FunctionDef, HttpMethod, Param};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, pk_name: &Ident, pk_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("exists");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &ReadTransaction, pk: &#pk_type) -> Result<bool, AppError> {
            let table_pk_11 = tx.open_table(#table)?;
            if table_pk_11.get(pk)?.is_some() {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    };
    let test_stream = Some(quote! {
        {
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let pk_value = #pk_type::default();
            let entity_exists = #entity_name::#fn_name(&read_tx, &pk_value).expect("Failed to check entity exists");
            assert!(entity_exists, "Entity is supposed to exist for the given PK");
        }
    });

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
            method: HttpMethod::HEAD,
            utoipa_responses: quote! { responses((status = OK)) },
            handler_impl_stream: quote! {
               Result<AppJson<bool>, AppError> {
                    state.db.begin_read().map_err(AppError::from).and_then(|tx| #entity_name::#fn_name(&tx, &#pk_name)).map(AppJson)
                }
            },
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), pk_name, pk_name),
        }),
        test_stream,
    }
}
