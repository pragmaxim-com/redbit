use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use crate::entity_macros::{Multiplicity, Pk, Relationship, Transient};
use crate::http_macros::{Endpoint, FunctionDef, Params, ReturnValue};

pub struct TransientMacros {
    pub struct_initializer: TokenStream,
}

pub struct DbRelationshipMacros {
    pub struct_initializer: TokenStream,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_def: FunctionDef,
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

impl DbRelationshipMacros {
    pub fn new(entity_ident: &Ident, pk_column: &Pk, rel: Relationship) -> DbRelationshipMacros {
        let pk_type = pk_column.field.tpe.clone();
        let pk_name = pk_column.field.name.clone();
        let child_name = &rel.field.name; // e.g., "transactions"
        let child_type = &rel.field.tpe; // e.g., the type `Transaction` from Vec<Transaction>
        let struct_initializer: TokenStream;
        let store_statement: TokenStream;
        let store_many_statement: TokenStream;
        let delete_statement: TokenStream;
        let delete_many_statement: TokenStream;
        let function_def: FunctionDef;
        let query_fn_name = format_ident!("get_{}", child_name);
        match rel.multiplicity {
            Multiplicity::OneToOne => {
                struct_initializer = quote! {
                    #child_name: #child_type::get(read_tx, pk)?.ok_or_else(|| AppError::NotFound(format!("Missing one-to-one child {:?}", pk)))?
                };
                store_statement = quote! {
                    let child = &instance.#child_name;
                    #child_type::store(&write_tx, child)?;
                };
                store_many_statement = quote! {
                    let children = instances.iter().map(|instance| instance.#child_name.clone()).collect();
                    #child_type::store_many(&write_tx, &children)?;
                };
                delete_statement = quote! {
                    #child_type::delete(&write_tx, pk)?;
                };
                delete_many_statement = quote! {
                    #child_type::delete_many(&write_tx, pks)?;
                };
                function_def = FunctionDef {
                    entity: entity_ident.clone(),
                    name: query_fn_name.clone(),
                    stream: quote! {
                        pub fn #query_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#child_type, AppError> {
                            #child_type::get(&read_tx, &pk).and_then(|opt| {
                                opt.ok_or_else(|| AppError::Internal(format!("No child found for pk: {:?}", pk)))
                            })
                        }
                    },
                    return_value: ReturnValue{ value_name: child_name.clone(), value_type: syn::parse_quote!(#child_type) },
                    endpoint: Some(Endpoint::Relation(Params { column_name: pk_name.clone(), column_type: pk_type.clone()})),
                };
            }
            Multiplicity::OneToMany => {
                struct_initializer = quote! {
                    #child_name: {
                        let (from, to) = pk.fk_range();
                        #child_type::range(read_tx, &from, &to)?
                    }
                };
                store_statement = quote! {
                    #child_type::store_many(&write_tx, &instance.#child_name)?;
                };
                store_many_statement = quote! {
                    let mut children: Vec<#child_type> = Vec::new();
                    for instance in instances.iter() {
                        children.extend_from_slice(&instance.#child_name)
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
                function_def = FunctionDef {
                    entity: entity_ident.clone(),
                    name: query_fn_name.clone(),
                    stream: quote! {
                        pub fn #query_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<Vec<#child_type>, AppError> {
                            let (from, to) = pk.fk_range();
                            #child_type::range(&read_tx, &from, &to)
                        }
                    },
                    return_value: ReturnValue{ value_name: child_name.clone(), value_type: syn::parse_quote!(Vec<#child_type>) },
                    endpoint: Some(Endpoint::Relation(Params { column_name: pk_name.clone(), column_type: pk_type.clone()})),
                };
            }
        }
        DbRelationshipMacros {
            struct_initializer,
            store_statement,
            store_many_statement,
            delete_statement,
            delete_many_statement,
            function_def
        }
    }
}
