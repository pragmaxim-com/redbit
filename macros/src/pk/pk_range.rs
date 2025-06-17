use crate::http::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, pk_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("pk_range");
    let fn_stream = quote! {
        fn #fn_name(tx: &::redbit::redb::WriteTransaction, from: &#pk_type, until: &#pk_type) -> Result<Vec<#pk_type>, AppError> {
            let table_pk_10 = tx.open_table(#table)?;
            let range = from.clone()..until.clone();
            let mut iter = table_pk_10.range(range)?;
            let mut results = Vec::new();
            while let Some(entry_res) = iter.next() {
                let pk = entry_res?.0.value();
                results.push(pk);
            }
            Ok(results)
        }
    };
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(Vec<#pk_type>),
        fn_stream,
        fn_call: quote! { #entity_name::#fn_name(&tx, &from, &until) },
        endpoint_def: None
    }

}