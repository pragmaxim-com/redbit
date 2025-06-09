use crate::http::{Endpoint, FunctionDef, ReturnValue};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let take_fn_name = format_ident!("take");
    let stream =
        quote! {
            pub fn #take_fn_name(read_tx: &::redb::ReadTransaction, n: u32) -> Result<Vec<#entity_type>, AppError> {
                let table_pk_6 = read_tx.open_table(#table)?;
                let mut iter = table_pk_6.iter()?;
                let mut results = Vec::new();
                let mut count = 0;

                while let Some(entry_res) = iter.next() {
                    if count >= n {
                        break;
                    }
                    let pk = entry_res?.0.value();
                    results.push(Self::compose(&read_tx, &pk)?);
                    count += 1;
                }

                Ok(results)
            }
        };
    FunctionDef {
        entity: entity_name.clone(),
        name: take_fn_name.clone(),
        stream,
        return_value: ReturnValue{ value_name: entity_name.clone(), value_type: syn::parse_quote!(Vec<#entity_type>) },
        endpoint: Some(Endpoint::Take),
    }
}