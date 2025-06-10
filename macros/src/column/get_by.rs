use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::http::{Endpoint, FunctionDef, GetByFlag, Params, ReturnValue};

pub fn get_by_dict_def(entity_name: &Ident, entity_type: &Type, column_name: &Ident, column_type: &Type, value_to_dict_pk: &Ident, dict_index_table: &Ident) -> FunctionDef {
    let get_by_name = format_ident!("get_by_{}", column_name);
    let stream = quote! {
        pub fn #get_by_name(
            read_tx: &::redb::ReadTransaction,
            val: &#column_type
        ) -> Result<Vec<#entity_type>, AppError> {
            let val2birth = read_tx.open_table(#value_to_dict_pk)?;
            let birth_guard = val2birth.get(val)?;
            let birth_id = match birth_guard {
                Some(g) => g.value().clone(),
                None => return Ok(Vec::new()),
            };
            let birth2pks = read_tx.open_multimap_table(#dict_index_table)?;
            let mut iter = birth2pks.get(&birth_id)?;
            let mut results = Vec::new();
            while let Some(x) = iter.next() {
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
            Ok(results)
        }
    };
    FunctionDef {
        entity: entity_name.clone(),
        name: get_by_name.clone(),
        stream,
        return_value: ReturnValue{ value_name: entity_name.clone(), value_type: syn::parse_quote!(Vec<#entity_type>) },
        endpoint: Some(Endpoint::GetBy(Params { column_name: column_name.clone(), column_type: column_type.clone()}, GetByFlag::Default)),
    }

}

pub fn get_by_index_def(entity_name: &Ident, entity_type: &Type, column_name: &Ident, column_type: &Type, table: &Ident) -> FunctionDef {
    let get_by_name = format_ident!("get_by_{}", column_name);
    let stream = quote! {
        pub fn #get_by_name(
            read_tx: &::redb::ReadTransaction,
            val: &#column_type
        ) -> Result<Vec<#entity_type>, AppError> {
            let mm_table = read_tx.open_multimap_table(#table)?;
            let mut iter = mm_table.get(val)?;
            let mut results = Vec::new();
            while let Some(x) = iter.next() {
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
            Ok(results)
        }
    };
    FunctionDef {
        entity: entity_name.clone(),
        name: get_by_name.clone(),
        stream,
        return_value: ReturnValue{ value_name: entity_name.clone(), value_type: syn::parse_quote!(Vec<#entity_type>) },
        endpoint: Some(Endpoint::GetBy(Params { column_name: column_name.clone(), column_type: column_type.clone()}, GetByFlag::Default)),
    }
}
