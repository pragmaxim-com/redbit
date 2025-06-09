use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::http::{Endpoint, FunctionDef, Params, ReturnValue};

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, table: &Ident) -> FunctionDef {
    let range_fn_name = format_ident!("range");
    let stream =
        quote! {
            pub fn #range_fn_name(read_tx: &::redb::ReadTransaction, from: &#pk_type, until: &#pk_type) -> Result<Vec<#entity_type>, AppError> {
                let table_pk_9 = read_tx.open_table(#table)?;
                let range = from.clone()..until.clone();
                let mut iter = table_pk_9.range(range)?;
                let mut results = Vec::new();
                while let Some(entry_res) = iter.next() {
                    let pk = entry_res?.0.value();
                    results.push(Self::compose(&read_tx, &pk)?);
                }
                Ok(results)
            }
        };
    FunctionDef {
        entity: entity_name.clone(),
        name: range_fn_name.clone(),
        stream,
        return_value: ReturnValue{ value_name: entity_name.clone(), value_type: syn::parse_quote!(Vec<#entity_type>) },
        endpoint: Some(Endpoint::RangeBy(Params { column_name: pk_name.clone(), column_type: pk_type.clone()})),
    }
}