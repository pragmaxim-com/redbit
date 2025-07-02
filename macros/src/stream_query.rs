use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;
use crate::column::DbColumnMacros;
use crate::field_parser::ColumnType;

pub fn stream_query_struct_macro(entity_name: &Ident, columns: &Vec<DbColumnMacros>) -> (Ident, TokenStream) {
    let stream_query_ident = format_ident!("{}StreamQuery", entity_name.to_string());

    let mut stream_query_field_defs = vec![];
    let mut stream_query_field_inits = vec![];
    for column in columns {
        let column_name = column.definition.field.name.clone();
        let column_type = column.definition.field.tpe.clone();
        match column.definition.col_type.clone() {
            ColumnType::Transient => {},
            ColumnType::IndexingOn { dictionary: _, range: _ } => {
                stream_query_field_defs.push(quote! { pub #column_name: Option<#column_type> });
                stream_query_field_inits.push(quote! { #column_name: Some(#column_type::default()) });
            },
            ColumnType::IndexingOff => {
                stream_query_field_defs.push(quote! { pub #column_name: Option<#column_type> });
                stream_query_field_inits.push(quote! { #column_name: Some(#column_type::default()) });
            },
        }
    }
    let token_stream = 
        quote! {
            #[derive(IntoParams, Serialize, Deserialize, Default)]
            pub struct #stream_query_ident {
                #(#stream_query_field_defs),*
            }
            impl #stream_query_ident {
                pub fn sample() -> Self {
                    Self {
                        #(#stream_query_field_inits),*
                    }
                }
            }
        };
    (stream_query_ident, token_stream)
}
/*
pub fn compose_entity_with_filter(entity_type: &Type, pk_name: &Ident, pk_type: &Type, stream_query_ident: &Ident) -> TokenStream {

    let column_locals: Vec<TokenStream> = column_struct_inits_by_name
        .iter()
        .map(|(field_ident, init_expr)| {
            quote! {
                let #field_ident = { #init_expr };
                if let Some(expected) = streaming_query.#field_ident.clone() {
                    if expected != #field_ident {
                        return Ok(None);
                    }
                }
            }
        })
        .collect();

    let col_idents: Vec<&Ident> = column_struct_inits_by_name.iter().map(|(ident, _)| ident).collect();

    quote! {
        fn compose_with_filter(tx: &ReadTransaction, pk: &#pk_type, streaming_query: #stream_query_ident) -> Result<Option<#entity_type>, AppError> {
            // First: fetch & filter every column, short‑circuit on mismatch
            #(#column_locals)*
            Ok(Some(#entity_type {
                #pk_name: pk.clone(),
                #(
                    #col_idents: #col_idents,
                )*
                #(#non_column_struct_inits),*
            }))
        }
    }
}*/