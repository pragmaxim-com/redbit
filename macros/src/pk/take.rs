use crate::http::ParamExtraction::FromQuery;
use crate::http::{EndpointDef, FunctionDef, HttpMethod, GetParam};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("take");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx: &::redb::ReadTransaction, n: u32) -> Result<Vec<#entity_type>, AppError> {
                let table_pk_6 = tx.open_table(#table)?;
                let mut iter = table_pk_6.iter()?;
                let mut results = Vec::new();
                let mut count = 0;

                while let Some(entry_res) = iter.next() {
                    if count >= n {
                        break;
                    }
                    let pk = entry_res?.0.value();
                    results.push(Self::compose(&tx, &pk)?);
                    count += 1;
                }

                Ok(results)
            }
        };

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        return_type: syn::parse_quote!(Vec<#entity_type>),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            param_extraction: FromQuery(vec![GetParam {
                name: format_ident!("take"),
                ty: syn::parse_quote!(u32),
                description: "Number of entities to return".to_string(),
            }]),
            method: HttpMethod::GET,
            endpoint: format!("/{}?take=", entity_name.to_string().to_lowercase()),
            fn_call: quote! { #entity_name::#fn_name(&tx, take) },
        })
    }

}