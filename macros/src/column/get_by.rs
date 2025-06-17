use crate::http::HttpParams::FromPath;
use crate::http::{EndpointDef, FunctionDef, HttpMethod, GetParam};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn get_by_dict_def(
    entity_name: &Ident,
    entity_type: &Type,
    column_name: &Ident,
    column_type: &Type,
    value_to_dict_pk: &Ident,
    dict_index_table: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("get_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(
            tx: &::redbit::redb::ReadTransaction,
            val: &#column_type
        ) -> Result<Vec<#entity_type>, AppError> {
            let val2birth = tx.open_table(#value_to_dict_pk)?;
            let birth_guard = val2birth.get(val)?;
            let birth_id = match birth_guard {
                Some(g) => g.value().clone(),
                None => return Ok(Vec::new()),
            };
            let birth2pks = tx.open_multimap_table(#dict_index_table)?;
            let mut iter = birth2pks.get(&birth_id)?;
            let mut results = Vec::new();
            while let Some(x) = iter.next() {
                let pk = x?.value();
                match Self::compose(&tx, &pk) {
                    Ok(item) => {
                        results.push(item);
                    }
                    Err(err) => {
                        return Err(AppError::Internal(err.to_string()));
                    }
                }
            }
            Ok(results)
        }
    };
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(Vec<#entity_type>),
        fn_stream,
        fn_call: quote! { #entity_name::#fn_name(&tx, &#column_name) },
        endpoint_def: Some(EndpointDef {
            params: FromPath(vec![GetParam {
                name: column_name.clone(),
                ty: column_type.clone(),
                description: "Secondary index column with dictionary".to_string(),
            }]),
            method: HttpMethod::GET(syn::parse_quote!(Vec<#entity_type>)),
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), column_name, column_name),
        }),
    }
}

pub fn get_by_index_def(entity_name: &Ident, entity_type: &Type, column_name: &Ident, column_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("get_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(
            tx: &::redbit::redb::ReadTransaction,
            val: &#column_type
        ) -> Result<Vec<#entity_type>, AppError> {
            let mm_table = tx.open_multimap_table(#table)?;
            let mut iter = mm_table.get(val)?;
            let mut results = Vec::new();
            while let Some(x) = iter.next() {
                let pk = x?.value();
                match Self::compose(&tx, &pk) {
                    Ok(item) => {
                        results.push(item);
                    }
                    Err(err) => {
                        return Err(AppError::Internal(err.to_string()));
                    }
                }
            }
            Ok(results)
        }
    };
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(Vec<#entity_type>),
        fn_stream,
        fn_call: quote! { #entity_name::#fn_name(&tx, &#column_name) },
        endpoint_def: Some(EndpointDef {
            params: FromPath(vec![GetParam {
                name: column_name.clone(),
                ty: column_type.clone(),
                description: "Secondary index column".to_string(),
            }]),
            method: HttpMethod::GET(syn::parse_quote!(Vec<#entity_type>)),
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), column_name, column_name),
        }),
    }
}
