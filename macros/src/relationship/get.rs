use crate::http::HttpParams::FromPath;
use crate::http::{EndpointDef, FunctionDef, HttpMethod, GetParam};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn o2o_def(entity_name: &Ident, child_name: &Ident, child_type: &Type, pk_name: &Ident, pk_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("get_{}", child_name);
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(#child_type),
        fn_stream: quote! {
            pub fn #fn_name(tx: &::redbit::redb::ReadTransaction, pk: &#pk_type) -> Result<#child_type, AppError> {
                #child_type::get(&tx, &pk).and_then(|opt| {
                    opt.ok_or_else(|| AppError::Internal(format!("No child found for pk: {:?}", pk)))
                })
            }
        },
        fn_call: quote! { #entity_name::#fn_name(&tx, &#pk_name) },
        endpoint_def: Some(EndpointDef {
            params: FromPath(vec![GetParam { name: pk_name.clone(), ty: pk_type.clone(), description: "Primary key".to_string() }]),
            method: HttpMethod::GET(child_type.clone()),
            endpoint: format!("/{}/{{{}}}/{}", entity_name.to_string().to_lowercase(), pk_name, child_name),
        })
    }
}

pub fn o2m_def(entity_name: &Ident, child_name: &Ident, child_type: &Type, pk_name: &Ident, pk_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("get_{}", child_name);
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(Vec<#child_type>),
        fn_stream: quote! {
            pub fn #fn_name(tx: &::redbit::redb::ReadTransaction, pk: &#pk_type) -> Result<Vec<#child_type>, AppError> {
                let (from, to) = pk.fk_range();
                #child_type::range(&tx, &from, &to)
            }
        },
        fn_call: quote! { #entity_name::#fn_name(&tx, &#pk_name) },
        endpoint_def: Some(EndpointDef {
            params: FromPath(vec![GetParam { name: pk_name.clone(), ty: pk_type.clone(), description: "Primary key".to_string() }]),
            method: HttpMethod::GET(syn::parse_quote!(Vec<#child_type>)),
            endpoint: format!("/{}/{{{}}}/{}", entity_name.to_string().to_lowercase(), pk_name, child_name),
        })
    }
}