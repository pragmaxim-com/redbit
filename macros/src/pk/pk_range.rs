use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::http::{FunctionDef, ReturnValue};

pub fn fn_def(entity_name: &Ident, pk_name: &Ident, pk_type: &Type, table: &Ident) -> FunctionDef {
    let pk_range_fn_name = format_ident!("pk_range");
    let stream = quote! {
        fn #pk_range_fn_name(write_tx: &::redb::WriteTransaction, from: &#pk_type, until: &#pk_type) -> Result<Vec<#pk_type>, AppError> {
            let table_pk_10 = write_tx.open_table(#table)?;
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
        entity: entity_name.clone(),
        name: pk_range_fn_name.clone(),
        stream,
        return_value: ReturnValue{ value_name: pk_name.clone(), value_type: syn::parse_quote!(Vec<#pk_type>) },
        endpoint: None,
    }

}