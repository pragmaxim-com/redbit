use crate::rest::HttpParams::FromPath;
use crate::rest::{EndpointDef, FunctionDef, GetParam, HttpMethod};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, pk_name: &Ident, pk_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("parent_key");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &::redbit::redb::ReadTransaction, pk: &#pk_type) -> Result<<#pk_type as ChildPointer>::Parent, AppError> {
            Ok(pk.parent().clone())
        }
    };
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(<#pk_type as ChildPointer>::Parent),
        is_sse: false,
        fn_stream,
        fn_call: quote! { #entity_name::#fn_name(&tx, &#pk_name) },
        endpoint_def: Some(EndpointDef {
            params: FromPath(vec![GetParam {
                name: pk_name.clone(),
                ty: pk_type.clone(),
                description: "Primary key of the owner entity".to_string(),
            }]),
            method: HttpMethod::GET,
            return_type: Some(pk_type.clone()),
            endpoint: format!("/{}/{}/{{{}}}/{}", entity_name.to_string().to_lowercase(), pk_name, pk_name, fn_name),
        }),
        test_stream: None
    }
}
