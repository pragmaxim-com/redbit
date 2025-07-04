use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn by_dict_def(
    entity_name: &Ident,
    pk_name: &Ident,
    pk_type: &Type,
    column_name: &Ident,
    column_type: &Type,
    value_to_dict_pk: &Ident,
    dict_index_table: &Ident,
) -> FunctionDef {
    let fn_name = format_ident!("get_{}s_by_{}", pk_name, column_name);
    let fn_stream = quote! {
        pub fn #fn_name(
            tx: &::redbit::redb::ReadTransaction,
            val: &#column_type
        ) -> Result<Vec<#pk_type>, AppError> {
            let val2birth = tx.open_table(#value_to_dict_pk)?;
            let birth_guard = val2birth.get(val)?;
            let birth_id = match birth_guard {
                Some(g) => g.value().clone(),
                None => return Ok(Vec::new()),
            };
            let birth2pks = tx.open_multimap_table(#dict_index_table)?;
            let mut iter = birth2pks.get(&birth_id)?;
            let mut results = Vec::new();
            while let Some(x) = iter.next() {
                let pk = x?.value();
                results.push(pk);
            }
            Ok(results)
        }
    };
    let test_stream = Some(quote! {
        {
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let entity_pks = #entity_name::#fn_name(&read_tx, &val).expect("Failed to get entity pks by dictionary index");
            let expected_entity_pks = vec![#pk_type::default()];
            assert_eq!(expected_entity_pks, entity_pks, "Expected entity pks to be returned for the given dictionary index");
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: None,
        test_stream
    }
}

pub fn by_index_def(entity_name: &Ident, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("get_{}s_by_{}", pk_name, column_name);
    let fn_stream = quote! {
        pub fn #fn_name(
            tx: &::redbit::redb::ReadTransaction,
            val: &#column_type
        ) -> Result<Vec<#pk_type>, AppError> {
            let mm_table = tx.open_multimap_table(#table)?;
            let mut iter = mm_table.get(val)?;
            let mut results = Vec::new();
            while let Some(x) = iter.next() {
                let pk = x?.value();
                results.push(pk);
            }
            Ok(results)
        }
    };
    let test_stream = Some(quote! {
        {
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let val = #column_type::default();
            let entity_pks = #entity_name::#fn_name(&read_tx, &val).expect("Failed to get entity pks by index");
            let expected_entity_pks = vec![#pk_type::default()];
            assert_eq!(expected_entity_pks, entity_pks, "Expected entity pks to be returned for the given index");
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream,
        endpoint_def: None,
        test_stream
    }
}