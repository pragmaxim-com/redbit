use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("take");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx: &ReadTransaction, n: usize) -> Result<Vec<#entity_type>, AppError> {
                let table_pk_6 = tx.open_table(#table)?;
                let mut iter = table_pk_6.iter()?;
                let mut results = Vec::new();
                let mut count: usize = 0;

                while let Some(entry_res) = iter.next() {
                    if count >= n {
                        break;
                    }
                    let pk = entry_res?.0.value();
                    results.push(Self::compose(&tx, &pk)?);
                    count += 1;
                }

                Ok(results)
            }
        };
    let test_stream = Some(quote! {
        {
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let n: usize = 2;
            let entities = #entity_name::#fn_name(&read_tx, n).expect("Failed to take entities");
            let expected_entities = #entity_type::sample_many(n);
            assert_eq!(entities, expected_entities, "Expected to take 2 entities");
        }
    });
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(Vec<#entity_type>),
        is_sse: false,
        fn_stream,
        fn_call: quote! { #entity_name::#fn_name(&tx, take) },
        endpoint_def: None,
        test_stream
    }

}