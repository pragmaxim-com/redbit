use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::http::{Endpoint, FunctionDef, GetByFlag, Params, ReturnValue};

pub fn fn_def(entity_name: &Ident, pk_name: &Ident, pk_type: &Type, table: &Ident) -> FunctionDef {
    let exists_fn_name = format_ident!("exists");
    let stream =
        quote! {
            pub fn #exists_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<bool, AppError> {
                let table_pk_11 = read_tx.open_table(#table)?;
                if table_pk_11.get(pk)?.is_some() {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        };
    FunctionDef {
        entity: entity_name.clone(),
        name: exists_fn_name.clone(),
        stream,
        return_value: ReturnValue { value_name: entity_name.clone(), value_type: syn::parse_quote!(bool) },
        endpoint: Some(Endpoint::GetBy(Params { column_name: pk_name.clone(), column_type: pk_type.clone()}, GetByFlag::Exists)),
    }
}