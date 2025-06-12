use crate::http::ParamExtraction::FromQuery;
use crate::http::{EndpointDef, FunctionDef};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn o2o_def(entity_name: &Ident, child_name: &Ident, child_type: &Type, pk_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("get_{}", child_name);
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        return_type: syn::parse_quote!(#child_type),
        fn_stream: quote! {
            pub fn #fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#child_type, AppError> {
                #child_type::get(&read_tx, &pk).and_then(|opt| {
                    opt.ok_or_else(|| AppError::Internal(format!("No child found for pk: {:?}", pk)))
                })
            }
        },
        endpoint_def: Some(EndpointDef {
            param_extraction: FromQuery(syn::parse_quote!(RequestByParams<#pk_type>)),
            method: format_ident!("get"),
            endpoint: format!("/{}/{{value}}/{}", entity_name.to_string().to_lowercase(), child_name.clone()),
            fn_call: quote! { #entity_name::#fn_name(&read_tx, &params.value) },
        })
    }
}

pub fn o2m_def(entity_name: &Ident, child_name: &Ident, child_type: &Type, pk_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("get_{}", child_name);
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        return_type: syn::parse_quote!(Vec<#child_type>),
        fn_stream: quote! {
            pub fn #fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<Vec<#child_type>, AppError> {
                let (from, to) = pk.fk_range();
                #child_type::range(&read_tx, &from, &to)
            }
        },
        endpoint_def: Some(EndpointDef {
            param_extraction: FromQuery(syn::parse_quote!(RequestByParams<#pk_type>)),
            method: format_ident!("get"),
            endpoint: format!("/{}/{{value}}/{}", entity_name.to_string().to_lowercase(), child_name.clone()),
            fn_call: quote! { #entity_name::#fn_name(&read_tx, &params.value) },
        })
    }
}