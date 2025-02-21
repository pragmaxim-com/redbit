use std::{env, fs};
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use crate::column_macros::ColumnMacros;
use crate::{Column, Indexing, Pk, Relationship};
use crate::pk_macros::PkMacros;
use crate::relationship_macros::RelationshipMacros;

pub struct EntityMacros {
    pub struct_name: Ident,
    pub pk_column: (Pk, PkMacros),
    pub columns: Vec<(Column, ColumnMacros)>,
    pub relationships: Vec<(Relationship, RelationshipMacros)>
}

impl EntityMacros {
    pub fn new(struct_name: Ident, pk_column: Pk, struct_columns: Vec<Column>, relationships: Vec<Relationship>) -> Result<EntityMacros, syn::Error> {
        let pk_name = &pk_column.field.name;
        let pk_type = &pk_column.field.tpe;
        let mut column_macros: Vec<(Column, ColumnMacros)> = Vec::new();
        for struct_column in struct_columns.into_iter() {
            let column_name = &struct_column.field.name.clone();
            let column_type = &struct_column.field.tpe.clone();
            match struct_column.indexing {
                Indexing::Off => {
                    column_macros.push((struct_column, ColumnMacros::simple(&struct_name, pk_name, pk_type, column_name, column_type)));
                }
                Indexing::On { dictionary: false, range } => {
                    column_macros.push((struct_column, ColumnMacros::indexed(&struct_name, pk_name, pk_type, column_name, column_type, range)));
                }
                Indexing::On { dictionary: true, range: false } => {
                    column_macros.push((struct_column, ColumnMacros::indexed_with_dict(&struct_name, pk_name, pk_type, column_name, column_type)));
                }
                Indexing::On { dictionary: true, range: true } => {
                    return Err(syn::Error::new(column_name.span(), "Range indexing on dictionary columns is not supported"))
                }
            }
        }
        // println!("Tables for {}:\n{}\n{}\n", struct_name, table_name_str, table_names.join("\n"));
        let pk_macros = PkMacros::new(&struct_name, &pk_column);
        let relationship_macros= RelationshipMacros::new(&pk_column, relationships);
        Ok(EntityMacros { struct_name, pk_column: (pk_column, pk_macros), columns: column_macros, relationships: relationship_macros })
    }

    pub fn expand(&self) -> TokenStream {
        let struct_ident = &self.struct_name;
        let (pk_column, pk_column_macros) = &self.pk_column;
        let pk_ident = pk_column.field.name.clone();
        let pk_type = pk_column.field.tpe.clone();
        let pk_table_definition = pk_column_macros.table_definition.clone();
        let pk_store_statement = pk_column_macros.store_statement.clone();
        let pk_query_function = pk_column_macros.query_function.clone();
        let pk_range_function = pk_column_macros.range_function.clone();

        let mut table_definitions = Vec::new();
        let mut store_statements = Vec::new();
        let mut struct_initializers = Vec::new();
        let mut query_functions = Vec::new();
        let mut range_functions = Vec::new();

        for (_, macros) in &self.columns {
            table_definitions.extend(macros.table_definitions.clone());
            store_statements.push(macros.store_statement.clone());
            struct_initializers.push(macros.struct_initializer.clone());
            query_functions.push(macros.query_function.clone());
            if let Some(range_fn) = &macros.range_function {
                range_functions.push(range_fn.clone());
            }
        }

        for (_, macros) in &self.relationships {
            store_statements.push(macros.store_statement.clone());
            struct_initializers.push(macros.struct_initializer.clone());
            query_functions.push(Some(macros.query_function.clone()));
        }

        // Build the final TokenStream
        let expanded = quote! {
            #pk_table_definition
            #(#table_definitions)*

            impl #struct_ident {
                #pk_range_function

                fn compose(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#struct_ident, DbEngineError> {
                    Ok(#struct_ident {
                        #pk_ident: pk.clone(),
                        #(#struct_initializers),*
                    })
                }

                #pk_query_function
                #(#query_functions)*
                #(#range_functions)*

                pub fn store(write_tx: &::redb::WriteTransaction, instance: &#struct_ident) -> Result<(), DbEngineError> {
                    #pk_store_statement
                    #(#store_statements)*
                    Ok(())
                }
                pub fn store_and_commit(db: &::redb::Database, instance: &#struct_ident) -> Result<(), DbEngineError> {
                    let write_tx = db.begin_write()?;
                    {
                        #pk_store_statement
                        #(#store_statements)*
                    }
                    write_tx.commit()?;
                    Ok(())
                }
            }
        };

        let formatted_str = match syn::parse2(expanded.clone()) {
            Ok(ast) => prettyplease::unparse(&ast),
            Err(_) => expanded.to_string(),
        };
        let _ = fs::write(env::temp_dir().join("redbit_macro.rs"), &formatted_str).unwrap();
        expanded
    }
}
