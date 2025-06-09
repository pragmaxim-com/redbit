use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::http::{Endpoint, FunctionDef, Params, ReturnValue};

pub fn o2o_def(entity_name: &Ident, child_name: &Ident, child_type: &Type, pk_name: &Ident, pk_type: &Type) -> FunctionDef {
    let query_fn_name = format_ident!("get_{}", child_name);
    FunctionDef {
        entity: entity_name.clone(),
        name: query_fn_name.clone(),
        stream: quote! {
            pub fn #query_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#child_type, AppError> {
                #child_type::get(&read_tx, &pk).and_then(|opt| {
                    opt.ok_or_else(|| AppError::Internal(format!("No child found for pk: {:?}", pk)))
                })
            }
        },
        return_value: ReturnValue{ value_name: child_name.clone(), value_type: syn::parse_quote!(#child_type) },
        endpoint: Some(Endpoint::Relation(Params { column_name: pk_name.clone(), column_type: pk_type.clone()})),
    }
}

pub fn o2m_def(entity_name: &Ident, child_name: &Ident, child_type: &Type, pk_name: &Ident, pk_type: &Type) -> FunctionDef {
    let query_fn_name = format_ident!("get_{}", child_name);
    FunctionDef {
        entity: entity_name.clone(),
        name: query_fn_name.clone(),
        stream: quote! {
            pub fn #query_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<Vec<#child_type>, AppError> {
                let (from, to) = pk.fk_range();
                #child_type::range(&read_tx, &from, &to)
            }
        },
        return_value: ReturnValue{ value_name: child_name.clone(), value_type: syn::parse_quote!(Vec<#child_type>) },
        endpoint: Some(Endpoint::Relation(Params { column_name: pk_name.clone(), column_type: pk_type.clone()})),
    }
}