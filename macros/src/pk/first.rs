use crate::http::ParamExtraction::FromQuery;
use crate::http::{EndpointDef, FunctionDef};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("first");
    let fn_stream = quote! {
        pub fn #fn_name(read_tx: &::redb::ReadTransaction) -> Result<Option<#entity_type>, AppError> {
            let table_pk_7 = read_tx.open_table(#table)?;
            if let Some((k, _)) = table_pk_7.first()? {
                return Self::compose(&read_tx, &k.value()).map(Some);
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
            param_extraction: FromQuery(syn::parse_quote!(FirstParams)),
            method: format_ident!("get"),
            endpoint: format!("/{}?first=", entity_name.to_string().to_lowercase()),
            fn_call: quote! { #entity_name::#fn_name(&read_tx) },
        })
    }
}