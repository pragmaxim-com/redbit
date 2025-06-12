use crate::http::ParamExtraction::FromPath;
use crate::http::{EndpointDef, FunctionDef};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, pk_name: &Ident, pk_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("exists");
    let fn_stream =
        quote! {
            pub fn #fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<bool, AppError> {
                let table_pk_11 = read_tx.open_table(#table)?;
                if table_pk_11.get(pk)?.is_some() {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        };
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        return_type: syn::parse_quote!(bool),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            param_extraction: FromPath(syn::parse_quote!(RequestByParams<#pk_type>)),
            method: format_ident!("head"),
            endpoint: format!("/{}/{}/{{value}}", entity_name.to_string().to_lowercase(), pk_name.clone()),
            fn_call: quote! { #entity_name::#fn_name(&read_tx, &params.value) },
        })
    }
}