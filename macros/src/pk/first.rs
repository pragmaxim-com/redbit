use crate::http::{Endpoint, FunctionDef, ReturnValue};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let take_fn_name = format_ident!("first");
    let stream = quote! {
        pub fn #take_fn_name(read_tx: &::redb::ReadTransaction) -> Result<Option<#entity_type>, AppError> {
            let table_pk_7 = read_tx.open_table(#table)?;
            if let Some((k, _)) = table_pk_7.first()? {
                return Self::compose(&read_tx, &k.value()).map(Some);
            }
            Ok(None)
        }
    };
    FunctionDef{
        entity: entity_name.clone(),
        name: take_fn_name.clone(),
        stream,
        return_value: ReturnValue{ value_name: entity_name.clone(), value_type: syn::parse_quote!(Option<#entity_type>) },
        endpoint: Some(Endpoint::First),
    }
}