use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::http::{Endpoint, FunctionDef, ReturnValue};

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let last_fn_name = format_ident!("last");
    let stream = quote! {
        pub fn #last_fn_name(read_tx: &::redb::ReadTransaction) -> Result<Option<#entity_type>, AppError> {
            let table_pk_8 = read_tx.open_table(#table)?;
            if let Some((k, _)) = table_pk_8.last()? {
                return Self::compose(&read_tx, &k.value()).map(Some);
            }
            Ok(None)
        }
    };
    FunctionDef {
        entity: entity_name.clone(),
        name: last_fn_name.clone(),
        stream,
        return_value: ReturnValue{ value_name: entity_name.clone(), value_type: syn::parse_quote!(Option<#entity_type>) },
        endpoint: Some(Endpoint::Last),
    }
}