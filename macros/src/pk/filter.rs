use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_type: &Type, table: &Ident, stream_query_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("filter");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &ReadTransaction, pk: &#pk_type, query: &#stream_query_type) -> Result<Option<#entity_type>, AppError> {
            let table_pk_5 = tx.open_table(#table)?;
            if table_pk_5.get(pk)?.is_some() {
                Ok(Self::compose_with_filter(&tx, pk, query)?)
            } else {
                Ok(None)
            }
        }
    };
    let test_fn_name = format_ident!("test_{}", fn_name);
    let test_stream = Some(quote! {
        #[tokio::test]
        async fn #test_fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let query = #stream_query_type::sample();
            let entity = #entity_name::#fn_name(&read_tx, &#pk_type::default(), &query).expect("Failed to get entity by PK").expect("Expected entity to exist");
            assert_eq!(entity, #entity_type::sample(), "Entity PK does not match the requested PK");

            let entity_opt = #entity_name::#fn_name(&read_tx, &#pk_type::default().next(), &query).expect("Failed to get entity by PK");
            assert_eq!(entity_opt, None, "Filter is set for default value");
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: None,
        test_stream,
    }
}
