use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::http::{Endpoint, FunctionDef, Params, ReturnValue};

pub fn range_by_index_def(entity_name: &Ident, entity_type: &Type, column_name: &Ident, column_type: &Type, table: &Ident) -> FunctionDef {
    let range_by_name = format_ident!("range_by_{}", column_name);
    let stream = quote! {
        pub fn #range_by_name(
            read_tx: &::redb::ReadTransaction,
            from: &#column_type,
            until: &#column_type
        ) -> Result<Vec<#entity_type>, AppError> {
            let mm_table = read_tx.open_multimap_table(#table)?;
            let range_iter = mm_table.range(from.clone()..until.clone())?;
            let mut results = Vec::new();
            for entry_res in range_iter {
                let (col_key, mut multi_iter) = entry_res?;
                while let Some(x) = multi_iter.next() {
                    let pk = x?.value();
                    match Self::compose(&read_tx, &pk) {
                        Ok(item) => {
                            results.push(item);
                        }
                        Err(err) => {
                            return Err(AppError::Internal(err.to_string()));
                        }
                    }
                }
            }
            Ok(results)
        }
    };
    FunctionDef {
        entity: entity_name.clone(),
        name: range_by_name.clone(),
        stream,
        return_value: ReturnValue{ value_name: entity_name.clone(), value_type: syn::parse_quote!(Vec<#entity_type>) },
        endpoint: Some(Endpoint::RangeBy(Params { column_name: column_name.clone(), column_type: column_type.clone()})),
    }
}