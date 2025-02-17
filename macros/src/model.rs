use std::{env, fs};
use syn::Type;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indexing {
    Off,
    On { dictionary: bool, range: bool },
}

pub enum ParsingResult {
    Pk(Pk),
    Column(Column),
}

pub struct Pk {
    pub pk_name: Ident,
    pub pk_type: Type,
    pub range: bool,
}

pub struct Column {
    pub column_name: Ident,
    pub column_type: Type,
    pub indexing: Indexing,
}

pub struct PkMacros {
    pub table_definition: TokenStream,
    pub store_statement: TokenStream,
    pub query_function: TokenStream,
    pub range_function: Option<TokenStream>,
}

impl PkMacros {
    pub fn new(struct_name: &Ident, pk_column: &Pk) -> Self {
        let table_ident = format_ident!("{}_{}", struct_name.to_string().to_uppercase(), pk_column.pk_name.to_string().to_uppercase());
        let table_name_str = table_ident.to_string();
        let pk_type = pk_column.pk_type.clone();
        let pk_name: Ident = pk_column.pk_name.clone();

        let table_definition = quote! {
            pub const #table_ident: ::redb::TableDefinition<'static, #pk_type, ()> = ::redb::TableDefinition::new(#table_name_str);
        };

        let store_statement = quote! {
            let mut table = write_txn.open_table(#table_ident)?;
            table.insert(&instance.#pk_name, ())?;
        };

        let get_fn_name = format_ident!("get_by_{}", pk_column.pk_name);
        let query_function = quote! {
            pub fn #get_fn_name(db: &::redb::Database, pk: &#pk_type) -> Result<#struct_name, DbEngineError> {
                let read_txn = match db.begin_read() {
                    Ok(txn) => txn,
                    Err(err) => return Err(DbEngineError::DbError(err.to_string())),
                };
                Self::compose(&read_txn, pk)
            }
        };

        let range_function = if pk_column.range {
            let range_fn_name = format_ident!("range_by_{}", pk_column.pk_name);
            Some(quote! {
                pub fn #range_fn_name(db: &::redb::Database, from: &#pk_type, to: &#pk_type) -> Result<Vec<#struct_name>, DbEngineError> {
                    let read_txn = match db.begin_read() {
                        Ok(txn) => txn,
                        Err(err) => return Err(DbEngineError::DbError(err.to_string())),
                    };
                    let table = read_txn.open_table(#table_ident)?;
                    let range = from.clone()..=to.clone();
                    let mut iter = table.range(range)?;
                    let mut results = Vec::new();
                    while let Some(entry_res) = iter.next() {
                        let pk = entry_res?.0.value();
                        results.push(Self::compose(&read_txn, &pk)?);
                    }
                    Ok(results)
                }
            })
        } else {
            None
        };

        PkMacros { table_definition, store_statement, query_function, range_function }
    }
}

pub struct ColumnMacros {
    pub table_definitions: Vec<TokenStream>,
    pub store_statement: TokenStream,
    pub struct_initializer: TokenStream,
    pub query_function: Option<TokenStream>,
    pub range_function: Option<TokenStream>,
}

pub struct StructMacros {
    pub struct_name: Ident,
    pub pk_column: (Pk, PkMacros),
    pub columns: Vec<(Column, ColumnMacros)>,
}

impl StructMacros {

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