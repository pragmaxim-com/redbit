use crate::http::ParamExtraction::FromPath;
use crate::http::{EndpointDef, FunctionDef, HttpMethod};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, pk_name: &Ident, pk_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("parent_pk");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<<#pk_type as ChildPointer>::Parent, AppError> {
                Ok(pk.parent().clone())
            }
        };
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        return_type: syn::parse_quote!(<#pk_type as ChildPointer>::Parent),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            param_extraction: FromPath(syn::parse_quote!(RequestByParams<#pk_type>)),
            method: HttpMethod::GET,
            endpoint: format!("/{}/{}/{{value}}/{}", entity_name.to_string().to_lowercase(), pk_name.clone(), fn_name),
            fn_call: quote! { #entity_name::#fn_name(&tx, &params.value) },
        })
    }
}