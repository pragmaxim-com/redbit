use crate::rest::HttpParams::FromQuery;
use crate::rest::{EndpointDef, FunctionDef, HttpMethod};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn range_by_index_def(entity_name: &Ident, entity_type: &Type, column_name: &Ident, column_type: &Type, table: &Ident, column_query: &Ident) -> FunctionDef {
    let fn_name = format_ident!("range_by_{}", column_name);
    let fn_stream = quote! {
        pub fn #fn_name(
            tx: &::redbit::redb::ReadTransaction,
            from: &#column_type,
            until: &#column_type
        ) -> Result<Vec<#entity_type>, AppError> {
            let mm_table = tx.open_multimap_table(#table)?;
            let range_iter = mm_table.range::<#column_type>(from..until)?;
            let mut results = Vec::new();
            for entry_res in range_iter {
                let (col_key, mut multi_iter) = entry_res?;
                while let Some(x) = multi_iter.next() {
                    let pk = x?.value();
                    match Self::compose(&tx, &pk) {
                        Ok(item) => {
                            results.push(item);
                        }
                        Err(err) => {
                            return Err(AppError::Internal(err.to_string()));
                        }
                    }
                }
            }
            Ok(results)
        }
    };
    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_return_type: syn::parse_quote!(Vec<#entity_type>),
        fn_stream,
        fn_call: quote! { #entity_name::#fn_name(&tx, &query.from, &query.until) },
        endpoint_def: Some(EndpointDef {
            params: FromQuery(syn::parse_quote!(#column_query)),
            method: HttpMethod::GET,
            return_type: Some(syn::parse_quote!(Vec<#entity_type>)),
            endpoint: format!("/{}/{}", entity_name.to_string().to_lowercase(), column_name.clone()),
        }),
    }
}
