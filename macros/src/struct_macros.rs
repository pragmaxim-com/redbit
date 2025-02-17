use std::{env, fs};
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use crate::column_macros::ColumnMacros;
use crate::{Column, Indexing, Pk};
use crate::pk_macros::PkMacros;

pub struct StructMacros {
    pub struct_name: Ident,
    pub pk_column: (Pk, PkMacros),
    pub columns: Vec<(Column, ColumnMacros)>,
}

impl StructMacros {
    pub fn new(struct_columns: Vec<Column>, struct_name: Ident, pk_column: Pk) -> Result<StructMacros, syn::Error> {
        let pk_name = &pk_column.pk_name;
        let pk_type = &pk_column.pk_type;
        let mut columns: Vec<(Column, ColumnMacros)> = Vec::new();
        for struct_column in struct_columns.into_iter() {
            let column_name = &struct_column.column_name.clone();
            let column_type = &struct_column.column_type.clone();
            match struct_column.indexing {
                Indexing::Off => {
                    columns.push((struct_column, ColumnMacros::simple(&struct_name, pk_name, pk_type, column_name, column_type)));
                }
                Indexing::On { dictionary: false, range } => {
                    columns.push((struct_column, ColumnMacros::indexed(&struct_name, pk_name, pk_type, column_name, column_type, range)));
                }
                Indexing::On { dictionary: true, range: false } => {
                    columns.push((struct_column, ColumnMacros::indexed_with_dict(&struct_name, pk_name, pk_type, column_name, column_type)));
                }
                Indexing::On { dictionary: true, range: true } => {
                    return Err(syn::Error::new(column_name.span(), "Range indexing on dictionary columns is not supported"))
                }
            }
        }

        // println!("Tables for {}:\n{}\n{}\n", struct_name, table_name_str, table_names.join("\n"));
        let pk_macros = PkMacros::new(&struct_name, &pk_column);
        Ok(StructMacros { struct_name, pk_column: (pk_column, pk_macros), columns })
    }

    pub fn expand(&self) -> TokenStream {
        let struct_ident = &self.struct_name;
        let (pk_column, pk_column_macros) = &self.pk_column;
        let pk_ident = pk_column.pk_name.clone();
        let pk_type = pk_column.pk_type.clone();
        let pk_table_definition = pk_column_macros.table_definition.clone();
        let pk_store_statement = pk_column_macros.store_statement.clone();
        let pk_query_function = pk_column_macros.query_function.clone();
        let pk_range_function = pk_column_macros.range_function.clone();

        let mut table_definitions = Vec::new();
        let mut store_statements = Vec::new();
        let mut struct_initializers = Vec::new();
        let mut query_functions = Vec::new();
        let mut range_functions = Vec::new();

        for (_, fm) in &self.columns {
            table_definitions.extend(fm.table_definitions.clone());
            store_statements.push(fm.store_statement.clone());
            struct_initializers.push(fm.struct_initializer.clone());
            query_functions.push(fm.query_function.clone());
            if let Some(range_fn) = &fm.range_function {
                range_functions.push(range_fn.clone());
            }
        }

        // Build the final TokenStream
        let expanded = quote! {
            #pk_table_definition
            #(#table_definitions)*

            impl #struct_ident {
                fn compose(read_txn: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#struct_ident, DbEngineError> {
                    Ok(#struct_ident {
                        #pk_ident: pk.clone(),
                        #(#struct_initializers),*
                    })
                }

                #pk_query_function
                #(#query_functions)*

                #pk_range_function

                #(#range_functions)*

                pub fn store(db: &::redb::Database, instance: &#struct_ident) -> Result<(), DbEngineError> {
                    let write_txn = db.begin_write()?;
                    {
                        #pk_store_statement
                        #(#store_statements)*
                    }
                    write_txn.commit()?;
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
