use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use crate::{Pk, Relationship};

pub struct RelationshipMacros {
    pub struct_initializer: TokenStream,
    pub store_statement: TokenStream,
    pub query_function: TokenStream,
}

impl RelationshipMacros {

    pub fn new(pk_column: &Pk, relationships: Vec<Relationship>) -> Vec<(Relationship, RelationshipMacros)> {
        let pk_type = pk_column.field.tpe.clone();
        let mut relationship_macros = Vec::new();
        for rel in relationships {
            let field_name = &rel.field.name; // e.g., "transactions"
            let child_type = &rel.field.tpe;  // e.g., the type `Transaction` from Vec<Transaction>

            let struct_init = quote! {
                #field_name: {
                    let (from, to) = pk.fk_range();
                    #child_type::range(read_tx, &from, &to)?
                }
            };
            let store_statement = quote! {
                for child in &instance.#field_name {
                    println!("{:?}", child);
                    #child_type::store(&write_tx, child)?;
                }
            };
            let query_fn_name = format_ident!("get_{}", field_name);
            let query_fn = quote! {
                pub fn #query_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<Vec<#child_type>, DbEngineError> {
                    let (from, to) = pk.fk_range();
                    #child_type::range(&read_tx, &from, &to)
                }
            };
            relationship_macros.push(
        (rel,
                RelationshipMacros {
                    struct_initializer: quote! { #struct_init },
                    store_statement: quote! { #store_statement },
                    query_function: quote! { #query_fn },
                }
                )
            )
        }
        relationship_macros
    }
}