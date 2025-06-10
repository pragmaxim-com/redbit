use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::http::{Endpoint, FunctionDef, GetByFlag, Params, ReturnValue};

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, table: &Ident) -> FunctionDef {
    let get_fn_name = format_ident!("get");
    let stream =
        quote! {
            pub fn #get_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<Option<#entity_type>, AppError> {
                let table_pk_5 = read_tx.open_table(#table)?;
                if table_pk_5.get(pk)?.is_some() {
                    Ok(Some(Self::compose(&read_tx, pk)?))
                } else {
                    Ok(None)
                }
            }
        };
    FunctionDef {
        entity: entity_name.clone(),
        name: get_fn_name.clone(),
        stream,
        return_value: ReturnValue { value_name: entity_name.clone(), value_type: syn::parse_quote!(Option<#entity_type>) },
        endpoint: Some(Endpoint::GetBy(Params { column_name: pk_name.clone(), column_type: pk_type.clone()}, GetByFlag::Default)),
    }
}