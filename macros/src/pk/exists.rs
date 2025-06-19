use crate::rest::HttpParams::FromPath;
use crate::rest::{EndpointDef, FunctionDef, GetParam, HttpMethod};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, pk_name: &Ident, pk_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("exists");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx: &::redbit::redb::ReadTransaction, pk: &#pk_type) -> Result<bool, AppError> {
                let table_pk_11 = tx.open_table(#table)?;
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
        fn_return_type: syn::parse_quote!(bool),
        fn_stream,
        fn_call: quote! { #entity_name::#fn_name(&tx, &#pk_name) },
        endpoint_def: Some(EndpointDef {
            params: FromPath(vec![GetParam { name: pk_name.clone(), ty: pk_type.clone(), description: "Primary key".to_string() }]),
            method: HttpMethod::HEAD,
            return_type: None,
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), pk_name, pk_name),
        })
    }
}