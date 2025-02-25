use crate::column_macros::ColumnMacros;
use crate::pk_macros::PkMacros;
use crate::relationship_macros::RelationshipMacros;
use crate::{Column, Indexing, Pk, Relationship};
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;

pub struct EntityMacros {
    pub struct_name: Ident,
    pub pk_column: (Pk, PkMacros),
    pub columns: Vec<(Column, ColumnMacros)>,
    pub relationships: Vec<(Relationship, RelationshipMacros)>,
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
        let relationship_macros = RelationshipMacros::new(&pk_column, relationships);
        Ok(EntityMacros { struct_name, pk_column: (pk_column, pk_macros), columns: column_macros, relationships: relationship_macros })
    }

    pub fn expand(&self) -> TokenStream {
        let struct_ident = &self.struct_name;
        let (pk_column, pk_column_macros) = &self.pk_column;
        let pk_ident = pk_column.field.name.clone();
        let pk_type = pk_column.field.tpe.clone();
        let pk_table_definition = pk_column_macros.table_definition.clone();
        let pk_store_statement = pk_column_macros.store_statement.clone();
        let pk_store_many_statement = pk_column_macros.store_many_statement.clone();
        let pk_delete_statement = pk_column_macros.delete_statement.clone();

        let mut table_definitions = Vec::new();
        let mut store_statements = Vec::new();
        let mut store_many_statements = Vec::new();
        let mut struct_initializers = Vec::new();
        let mut delete_statements = Vec::new();
        let mut functions = Vec::new();
        functions.extend(pk_column_macros.functions.clone());

        for (_, macros) in &self.columns {
            table_definitions.extend(macros.table_definitions.clone());
            store_statements.push(macros.store_statement.clone());
            store_many_statements.push(macros.store_many_statement.clone());
            struct_initializers.push(macros.struct_initializer.clone());
            functions.extend(macros.functions.clone());
            delete_statements.push(macros.delete_statement.clone());
        }

        for (_, macros) in &self.relationships {
            store_statements.push(macros.store_statement.clone());
            store_many_statements.push(macros.store_many_statement.clone());
            struct_initializers.push(macros.struct_initializer.clone());
            functions.push(macros.query_function.clone());
            delete_statements.push(macros.delete_statement.clone());
        }
        let function_macros: Vec<TokenStream> = functions.into_iter().map(|f| f.1).collect::<Vec<_>>();
        let expanded = quote! {
            #pk_table_definition
            #(#table_definitions)*

            impl #struct_ident {
                #(#function_macros)*

                fn compose(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#struct_ident, DbEngineError> {
                    Ok(#struct_ident {
                        #pk_ident: pk.clone(),
                        #(#struct_initializers),*
                    })
                }

                pub fn delete(write_tx: &::redb::WriteTransaction, pk: &#pk_type) -> Result<(), DbEngineError> {
                    #pk_delete_statement
                    #(#delete_statements)*
                    Ok(())
                }

                pub fn delete_and_commit(db: &::redb::Database, pk: &#pk_type) -> Result<(), DbEngineError> {
                    let write_tx = db.begin_write()?;
                    {
                        #pk_delete_statement
                        #(#delete_statements)*
                    }
                    write_tx.commit()?;
                    Ok(())
                }

                pub fn store_many(write_tx: &::redb::WriteTransaction, instances: &Vec<#struct_ident>) -> Result<(), DbEngineError> {
                    #pk_store_many_statement
                    #(#store_many_statements)*
                    Ok(())
                }

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
        let path = env::temp_dir().join("redbit_macro.rs");
        let mut file = OpenOptions::new().create(true).append(true).open(&path).unwrap();
        file.write_all(formatted_str.as_bytes()).unwrap();
        expanded
    }
}
