use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("first");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &::redbit::redb::ReadTransaction) -> Result<Option<#entity_type>, AppError> {
            let table_pk_7 = tx.open_table(#table)?;
            if let Some((k, _)) = table_pk_7.first()? {
                return Self::compose(&tx, &k.value()).map(Some);
            }
            Ok(None)
        }
    };
    let test_stream = Some(quote! {
        {
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let entity = #entity_name::first(&read_tx).expect("Failed to get first entity by PK").expect("Expected first entity to exist");
            let expected_enity = #entity_type::sample();
            assert_eq!(entity, expected_enity, "First entity does not match expected");
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(Option<#entity_type>),
        is_sse: false,
        fn_stream,
        fn_call: quote! { #entity_name::#fn_name(&tx) },
        endpoint_def: None,
        test_stream
    }
}
