use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use crate::entity_macros::{Multiplicity, Pk, Relationship, Transient};

pub struct TransientMacros {
    pub struct_initializer: TokenStream,
}

pub struct RelationshipMacros {
    pub struct_initializer: TokenStream,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub query_function: (String, TokenStream),
}

impl TransientMacros {
    pub fn new(transients: Vec<Transient>) -> Vec<(Transient, TransientMacros)> {
        let mut transient_macros: Vec<(Transient, TransientMacros)> = Vec::new();
        for transient in transients {
            let field_name = &transient.field.name;
            let field_type = &transient.field.tpe;
            let struct_initializer = quote! {
                #field_name: <#field_type as Default>::default()
            };
            transient_macros.push((transient, TransientMacros { struct_initializer}))
        }
        transient_macros
    }
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
            let delete_many_statement: TokenStream;
            let query_function: (String, TokenStream);
            match rel.multiplicity {
                Multiplicity::OneToOne => {
                    struct_initializer = quote! {
                        #field_name: #child_type::get(read_tx, pk)?.expect("Missing one-to-one child")
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
                    delete_many_statement = quote! {
                        #child_type::delete_many(&write_tx, pks)?;
                    };
                    let query_fn_name = format_ident!("get_{}", field_name);
                    query_function = (
                        query_fn_name.to_string(),
                        quote! {
                            pub fn #query_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#child_type, DbEngineError> {
                                #child_type::get(&read_tx, &pk).and_then(|opt| {
                                    opt.ok_or_else(|| DbEngineError::DbError(format!("No child found for pk: {:?}", pk)))
                                })
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
                        let child_pks = #child_type::pk_range(&write_tx, &from, &to)?;
                        #child_type::delete_many(&write_tx, &child_pks)?;
                    };
                    delete_many_statement = quote! {
                        let mut children = Vec::new();
                        for pk in pks.iter() {
                            let (from, to) = pk.fk_range();
                            let child_pks = #child_type::pk_range(&write_tx, &from, &to)?;
                            children.extend_from_slice(&child_pks);
                        }
                        #child_type::delete_many(&write_tx, &children)?;
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
            relationship_macros.push((rel, RelationshipMacros { struct_initializer, store_statement, store_many_statement, delete_statement, delete_many_statement, query_function }))
        }
        relationship_macros
    }
}
