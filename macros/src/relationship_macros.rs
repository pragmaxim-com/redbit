use crate::{Multiplicity, Pk, Relationship};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

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
            let child_type = &rel.field.tpe; // e.g., the type `Transaction` from Vec<Transaction>
            let struct_initializer: TokenStream;
            let store_statement: TokenStream;
            let query_function: TokenStream;
            match rel.multiplicity {
                Multiplicity::OneToOne => {
                    struct_initializer = quote! {
                        #field_name: {
                            #child_type::get(read_tx, &pk)?
                        }
                    };
                    store_statement = quote! {
                        let child = &instance.#field_name;
                        #child_type::store(&write_tx, child)?;
                    };
                    let query_fn_name = format_ident!("get_{}", field_name);
                    query_function = quote! {
                        pub fn #query_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#child_type, DbEngineError> {
                            #child_type::get(&read_tx, &pk)
                        }
                    };
                }
                Multiplicity::OneToMany => {
                    struct_initializer = quote! {
                        #field_name: {
                            let (from, to) = pk.fk_range();
                            #child_type::range(read_tx, &from, &to)?
                        }
                    };
                    store_statement = quote! {
                        for child in &instance.#field_name {
                            #child_type::store(&write_tx, child)?;
                        }
                    };
                    let query_fn_name = format_ident!("get_{}", field_name);
                    query_function = quote! {
                        pub fn #query_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<Vec<#child_type>, DbEngineError> {
                            let (from, to) = pk.fk_range();
                            #child_type::range(&read_tx, &from, &to)
                        }
                    };
                }
            }
            relationship_macros.push((rel, RelationshipMacros { struct_initializer, store_statement, query_function }))
        }
        relationship_macros
    }
}
