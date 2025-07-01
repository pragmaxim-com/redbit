use crate::rest::HttpParams::FromQuery;
use crate::rest::{FunctionDef, HttpMethod};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;
use crate::endpoint::EndpointDef;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, table: &Ident, column_query: &Ident) -> FunctionDef {
    let fn_name = format_ident!("range");
    let fn_stream =
        quote! {
            pub fn #fn_name(tx: &ReadTransaction, from: &#pk_type, until: &#pk_type) -> Result<Vec<#entity_type>, AppError> {
                let table_pk_9 = tx.open_table(#table)?;
                let range = from..until;
                let mut iter = table_pk_9.range::<#pk_type>(range)?;
                let mut results = Vec::new();
                while let Some(entry_res) = iter.next() {
                    let pk = entry_res?.0.value();
                    results.push(Self::compose(&tx, &pk)?);
                }
                Ok(results)
            }
        };
    let test_stream = Some(quote! {
        {
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let from_value = #pk_type::default();
            let until_value = #pk_type::default().next().next().next();
            let entities = #entity_name::#fn_name(&read_tx, &from_value, &until_value).expect("Failed to get entities by range");
            let expected_entities = #entity_type::sample_many(entity_count);
            assert_eq!(entities, expected_entities, "Expected entities to be returned for the given range");
        }
    });

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(Vec<#entity_type>),
        is_sse: false,
        fn_stream,
        fn_call: quote! { #entity_name::#fn_name(&tx, &query.from, &query.until) },
        endpoint_def: Some(EndpointDef {
            params: vec![FromQuery(syn::parse_quote!(#column_query))],
            method: HttpMethod::GET,
            return_type: Some(syn::parse_quote!(Vec<#entity_type>)),
            endpoint: format!("/{}/{}", entity_name.to_string().to_lowercase(), pk_name.clone()),
        }),
        test_stream
    }
}