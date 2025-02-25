use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use crate::entity_macros::{Multiplicity, Pk, Relationship};

pub struct RelationshipMacros {
    pub struct_initializer: TokenStream,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub query_function: (String, TokenStream),
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
            let store_many_statement: TokenStream;
            let delete_statement: TokenStream;
            let query_function: (String, TokenStream);
            match rel.multiplicity {
                Multiplicity::OneToOne => {
                    struct_initializer = quote! {
                        #field_name: {
                            #child_type::get(read_tx, pk)?.unwrap()
                        }
                    };
                    store_statement = quote! {
                        let child = &instance.#field_name;
                        #child_type::store(&write_tx, child)?;
                    };
                    store_many_statement = quote! {
                        let children = instances.iter().map(|instance| instance.#field_name.clone()).collect();
                        #child_type::store_many(&write_tx, &children)?;
                    };
                    delete_statement = quote! {
                        #child_type::delete(&write_tx, pk)?;
                    };
                    let query_fn_name = format_ident!("get_{}", field_name);
                    query_function = (
                        query_fn_name.to_string(),
                        quote! {
                            pub fn #query_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<Option<#child_type>, DbEngineError> {
                                #child_type::get(&read_tx, &pk)
                            }
                        },
                    );
                }
                Multiplicity::OneToMany => {
                    struct_initializer = quote! {
                        #field_name: {
                            let (from, to) = pk.fk_range();
                            #child_type::range(read_tx, &from, &to)?
                        }
                    };
                    store_statement = quote! {
                        #child_type::store_many(&write_tx, &instance.#field_name)?;
                    };
                    store_many_statement = quote! {
                        let mut children: Vec<#child_type> = Vec::new();
                        for instance in instances.iter() {
                            children.extend_from_slice(&instance.#field_name)
                        };
                        #child_type::store_many(&write_tx, &children)?;
                    };
                    delete_statement = quote! {
                        let (from, to) = pk.fk_range();
                        for child_pk in #child_type::pk_range(&write_tx, &from, &to)? {
                            #child_type::delete(&write_tx, &child_pk)?
                        }
                    };
                    let query_fn_name = format_ident!("get_{}", field_name);
                    query_function = (
                        query_fn_name.to_string(),
                        quote! {
                            pub fn #query_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<Vec<#child_type>, DbEngineError> {
                                let (from, to) = pk.fk_range();
                                #child_type::range(&read_tx, &from, &to)
                            }
                        },
                    );
                }
            }
            relationship_macros.push((rel, RelationshipMacros { struct_initializer, store_statement, store_many_statement, delete_statement, query_function }))
        }
        relationship_macros
    }
}
