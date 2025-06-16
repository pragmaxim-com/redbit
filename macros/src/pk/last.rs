use crate::http::ParamExtraction::FromQuery;
use crate::http::{EndpointDef, FunctionDef, HttpMethod, GetParam};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("last");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &::redb::ReadTransaction) -> Result<Option<#entity_type>, AppError> {
            let table_pk_8 = tx.open_table(#table)?;
            if let Some((k, _)) = table_pk_8.last()? {
                return Self::compose(&tx, &k.value()).map(Some);
            }
            Ok(None)
        }
    };
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        return_type: syn::parse_quote!(Option<#entity_type>),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            param_extraction: FromQuery(vec![GetParam {
                name: format_ident!("last"),
                ty: syn::parse_quote!(bool),
                description: "Fetch the last entity".to_string(),
            }]),
            method: HttpMethod::GET,
            endpoint: format!("/{}?last=", entity_name.to_string().to_lowercase()),
            fn_call: quote! { #entity_name::#fn_name(&tx) },
        })
    }
}