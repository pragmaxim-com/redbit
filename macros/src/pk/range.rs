use crate::http::ParamExtraction::FromQuery;
use crate::http::{EndpointDef, FunctionDef, HttpMethod, GetParam};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("range");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx: &::redb::ReadTransaction, from: &#pk_type, until: &#pk_type) -> Result<Vec<#entity_type>, AppError> {
                let table_pk_9 = tx.open_table(#table)?;
                let range = from.clone()..until.clone();
                let mut iter = table_pk_9.range(range)?;
                let mut results = Vec::new();
                while let Some(entry_res) = iter.next() {
                    let pk = entry_res?.0.value();
                    results.push(Self::compose(&tx, &pk)?);
                }
                Ok(results)
            }
        };
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        return_type: syn::parse_quote!(Vec<#entity_type>),
        fn_stream,
        endpoint_def: Some(EndpointDef {
            param_extraction: FromQuery(vec![
                GetParam { name: format_ident!("from"), ty: pk_type.clone(), description: "Range from inclusive".to_string() },
                GetParam { name: format_ident!("until"), ty: pk_type.clone(), description: "Range until exclusive".to_string() },
            ]),
            method: HttpMethod::GET,
            endpoint: format!("/{}/{}?from=&until=", entity_name.to_string().to_lowercase(), pk_name.clone()),
            fn_call: quote! { #entity_name::#fn_name(&tx, &from, &until) },
        })
    }
}