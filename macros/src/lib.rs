extern crate proc_macro;

use proc_macro::TokenStream;
use std::{env, fs};
use proc_macro2::{Ident, Span};
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput, Data, Fields, Field, Type, spanned::Spanned, punctuated::Punctuated, token::Comma};

struct Column {
    column_name: Ident,
    column_type: Type,
    indexing: Indexing,
}

struct ColumnMacros {
    table_definitions: Vec<proc_macro2::TokenStream>,
    store_statement: proc_macro2::TokenStream,
    struct_initializer: proc_macro2::TokenStream,
    query_function: proc_macro2::TokenStream,
}

struct StructMacros {
    struct_name: Ident,
    pk_name: Ident,
    pk_type: Type,
    columns: Vec<(Column, ColumnMacros)>,
}

impl StructMacros {
    pub fn expand(&self) -> proc_macro2::TokenStream {
        let struct_ident = &self.struct_name;
        let pk_ident = &self.pk_name;
        let pk_type = &self.pk_type;

        let mut table_definitions = Vec::new();
        let mut store_statements = Vec::new();
        let mut struct_initializers = Vec::new();
        let mut query_functions = Vec::new();

        for (_, fm) in &self.columns {
            table_definitions.extend(fm.table_definitions.clone());
            store_statements.push(fm.store_statement.clone());
            struct_initializers.push(fm.struct_initializer.clone());
            query_functions.push(fm.query_function.clone());
        }

        // Build the final TokenStream
        let expanded = quote! {
            #(#table_definitions)*

            impl #struct_ident {
                fn compose(read_txn: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#struct_ident, DbEngineError> {
                    Ok(#struct_ident {
                        #pk_ident: pk.clone(),
                        #(#struct_initializers),*
                    })
                }
                #(#query_functions)*
                pub fn store(db: &::redb::Database, instance: &#struct_ident) -> Result<(), DbEngineError> {
                    let write_txn = db.begin_write()?;
                    {
                        #(#store_statements)*
                    }
                    write_txn.commit()?;
                    Ok(())
                }
            }

        };

        let formatted_str = match syn::parse2(expanded.clone()) {
            Ok(ast) => {
                prettyplease::unparse(&ast)
            }
            Err(_) => {
                expanded.to_string()
            }
        };
        let _ = fs::write(env::temp_dir().join("redbit_macro.rs"), &formatted_str).unwrap();
        expanded
    }
}


#[derive(Debug, PartialEq, Eq)]
enum Indexing {
    Off,
    On { dictionary: bool },
}

fn get_named_fields(ast: &DeriveInput) -> Result<Punctuated<Field, Comma>, syn::Error> {
    match &ast.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(columns_named) => Ok(columns_named.named.clone()),
            _ => Err(syn::Error::new(
                ast.span(),
                "`#[derive(Redbit)]` only supports structs with named columns.",
            )),
        },
        _ => Err(syn::Error::new(
            ast.span(),
            "`#[derive(Redbit)]` can only be applied to structs.",
        )),
    }
}

fn parse_struct_columns(
    columns: &Punctuated<Field, Comma>,
    span: &Span,
) -> Result<(Ident, Type, Vec<Column>), syn::Error> {
    let mut pk_column: Option<(Ident, Type)> = None;
    let mut parsed_columns = Vec::new();

    for column in columns.iter() {
        let column_name = match &column.ident {
            Some(ident) => ident.clone(),
            None => continue, // Skip unnamed columns
        };
        let column_type = column.ty.clone();
        let mut indexing = Indexing::Off;
        let mut has_annotation = false;

        for attr in &column.attrs {
            // If it's #[pk], set pk_column
            if attr.path().is_ident("pk") {
                if pk_column.is_some() {
                    return Err(syn::Error::new(
                        column.span(),
                        "Multiple `#[pk]` columns found; only one is allowed",
                    ));
                }
                pk_column = Some((column_name.clone(), column_type.clone()));
                has_annotation = true;
            }
            // If it's #[column(...)] parse indexing/dictionary
            if attr.path().is_ident("column") {
                has_annotation = true;
                let _ = attr.parse_nested_meta(|nested| {
                    if nested.path.is_ident("index") {
                        indexing = Indexing::On { dictionary: false };
                    }
                    if nested.path.is_ident("dictionary") {
                        indexing = Indexing::On { dictionary: true };
                    }
                    Ok(())
                });
            }
        }

        // If neither #[pk] nor #[column(...)] is found => error
        if !has_annotation {
            return Err(syn::Error::new(
                column.span(),
                "All columns must have a #[column] or #[pk] annotation",
            ));
        }

        // If this column is not the pk column, store it in parsed_columns
        if pk_column.as_ref().map(|(pk_ident, _)| pk_ident) != Some(&column_name) {
            parsed_columns.push(Column {
                column_name,
                column_type,
                indexing,
            });
        }
    }

    // Ensure exactly one pk is found
    let (pk_ident, pk_type) = pk_column.ok_or_else(|| {
        syn::Error::new(
            *span,
            "`#[pk]` attribute not found on any column. Exactly one column must have `#[pk]`.",
        )
    })?;

    // Ensure there's at least one #[column(...)]
    // If parsed_columns is empty => only pk was found => fail
    if parsed_columns.is_empty() {
        return Err(syn::Error::new(
            *span,
            "No #[column(...)] fields found. You must have at least one field with #[column].",
        ));
    }

    Ok((pk_ident, pk_type, parsed_columns))
}

fn generate_struct_macros(
    struct_columns: Vec<Column>,
    struct_name: Ident,
    pk_name: Ident,
    pk_type: Type,
) -> StructMacros {
    let mut columns: Vec<(Column, ColumnMacros)> = Vec::new();
    let mut table_names: Vec<String> = Vec::new();
    for struct_column in struct_columns.into_iter() {
        let mut table_definitions: Vec<proc_macro2::TokenStream> = Vec::new();

        let store_statement: proc_macro2::TokenStream;
        let struct_initializer: proc_macro2::TokenStream;
        let query_function: proc_macro2::TokenStream;

        let column_name = &struct_column.column_name;
        let column_type  = &struct_column.column_type;
        let column_str  = column_name.to_string();
        match struct_column.indexing {
            Indexing::Off => {
                let table_ident = format_ident!("{}_{}_BY_{}", struct_name.to_string().to_uppercase(), column_name.to_string().to_uppercase(), pk_name.to_string().to_uppercase());
                let table_name_str = table_ident.to_string();
                table_names.push(table_name_str.clone());
                table_definitions.push(quote! {
                        pub const #table_ident: ::redb::TableDefinition<'static, #pk_type, #column_type> = ::redb::TableDefinition::new(#table_name_str);
                    });
                store_statement = quote! {
                        let mut table = write_txn.open_table(#table_ident)?;
                        table.insert(&instance.#pk_name, &instance.#column_name)?;
                    };
                struct_initializer = quote! {
                        #column_name: {
                            let table = read_txn.open_table(#table_ident)?;
                            let guard = table.get(pk)?;
                            guard.unwrap().value()
                        }
                    };
                let get_fn_name = format_ident!("get_by_{}", pk_name);
                query_function = quote! {
                    pub fn #get_fn_name(db: &::redb::Database, pk: &#pk_type) -> Result<#struct_name, DbEngineError> {
                        let read_txn = match db.begin_read() {
                            Ok(txn) => txn,
                            Err(err) => return Err(DbEngineError::DbError(err.to_string())),
                        };
                        Self::compose(&read_txn, pk)
                    }
                };
            }
            Indexing::On { dictionary: false } => {
                let table_ident = format_ident!("{}_{}_BY_{}", struct_name.to_string().to_uppercase(), column_name.to_string().to_uppercase(), pk_name.to_string().to_uppercase());
                let table_name_str = table_ident.to_string();
                table_names.push(table_name_str.clone());
                table_definitions.push(quote! {
                        pub const #table_ident: ::redb::TableDefinition<'static, #pk_type, #column_type> = ::redb::TableDefinition::new(#table_name_str);
                    });

                let index_table_ident = format_ident!("{}_{}_INDEX", struct_name.to_string().to_uppercase(), column_str.to_string().to_uppercase());
                let index_table_name_str = index_table_ident.to_string();
                table_names.push(index_table_name_str.clone());
                table_definitions.push(quote! {
                        pub const #index_table_ident: ::redb::MultimapTableDefinition<'static, #column_type, #pk_type> = ::redb::MultimapTableDefinition::new(#index_table_name_str);
                    });
                store_statement = quote! {
                        let mut table = write_txn.open_table(#table_ident)?;
                        table.insert(&instance.#pk_name, &instance.#column_name)?;

                        let mut mm = write_txn.open_multimap_table(#index_table_ident)?;
                        mm.insert(&instance.#column_name, &instance.#pk_name)?;
                    };
                struct_initializer = quote! {
                        #column_name: {
                            let table = read_txn.open_table(#table_ident)?;
                            let guard = table.get(pk)?;
                            guard.unwrap().value()
                        }
                    };
                let get_fn_name = format_ident!("get_by_{}", column_name);
                query_function = quote! {
                        pub fn #get_fn_name(
                            db: &::redb::Database,
                            val: &#column_type
                        ) -> Result<Vec<#struct_name>, DbEngineError> {
                            let read_txn = db.begin_read()?;
                            let mm_table = read_txn.open_multimap_table(#index_table_ident)?;
                            let mut iter = mm_table.get(val)?;
                            let mut results = Vec::new();
                            while let Some(x) = iter.next() {
                                let pk = x?.value();
                                match Self::compose(&read_txn, &pk) {
                                    Ok(item) => {
                                        results.push(item);
                                    }
                                    Err(err) => {
                                        return Err(DbEngineError::DbError(err.to_string()));
                                    }
                                }
                            }
                            Ok(results)
                        }
                    };
            }
            Indexing::On { dictionary: true } => {
                let table_dict_pk_by_pk_ident = format_ident!("{}_{}_DICT_PK_BY_{}", struct_name.to_string().to_uppercase(), column_str.to_string().to_uppercase(), pk_name.to_string().to_uppercase());
                let table_dict_pk_by_pk_str = table_dict_pk_by_pk_ident.to_string();
                let table_value_to_dict_pk_ident = format_ident!("{}_{}_TO_DICT_PK", struct_name.to_string().to_uppercase(), column_str.to_string().to_uppercase());
                let table_value_to_dict_pk_str = table_value_to_dict_pk_ident.to_string();
                let table_value_by_dict_pk_ident = format_ident!("{}_{}_BY_DICT_PK", struct_name.to_string().to_uppercase(), column_str.to_string().to_uppercase());
                let table_value_by_dict_pk_str = table_value_by_dict_pk_ident.to_string();
                let table_dict_index_ident = format_ident!("{}_{}_DICT_INDEX", struct_name.to_string().to_uppercase(), column_str.to_string().to_uppercase());
                let table_dict_index_str = table_dict_index_ident.to_string();

                table_names.push(table_dict_pk_by_pk_str.clone());
                table_definitions.push(quote! {
                        pub const #table_dict_pk_by_pk_ident: ::redb::TableDefinition<'static, #pk_type, #pk_type> = ::redb::TableDefinition::new(#table_dict_pk_by_pk_str);
                    });
                table_names.push(table_value_to_dict_pk_str.clone());
                table_definitions.push(quote! {
                        pub const #table_value_to_dict_pk_ident: ::redb::TableDefinition<'static, #column_type, #pk_type> = ::redb::TableDefinition::new(#table_value_to_dict_pk_str);
                    });
                table_names.push(table_value_by_dict_pk_str.clone());
                table_definitions.push(quote! {
                        pub const #table_value_by_dict_pk_ident: ::redb::TableDefinition<'static, #pk_type, #column_type> = ::redb::TableDefinition::new(#table_value_by_dict_pk_str);
                    });
                table_names.push(table_dict_index_str.clone());
                table_definitions.push(quote! {
                        pub const #table_dict_index_ident: ::redb::MultimapTableDefinition<'static, #pk_type, #pk_type>= ::redb::MultimapTableDefinition::new(#table_dict_index_str);
                    });
                store_statement = quote! {
                        let mut dict_pk_by_pk       = write_txn.open_table(#table_dict_pk_by_pk_ident)?;
                        let mut value_to_dict_pk    = write_txn.open_table(#table_value_to_dict_pk_ident)?;
                        let mut value_by_dict_pk    = write_txn.open_table(#table_value_by_dict_pk_ident)?;
                        let mut dict_index          = write_txn.open_multimap_table(#table_dict_index_ident)?;

                        let (birth_id, newly_created) = {
                            let existing_guard = value_to_dict_pk.get(&instance.#column_name)?;
                            if let Some(guard) = existing_guard {
                                (guard.value().clone(), false)
                            } else {
                                let new_birth = instance.#pk_name.clone();
                                (new_birth, true)
                            }
                        };

                        if newly_created {
                            value_to_dict_pk.insert(&instance.#column_name, &birth_id)?;
                            value_by_dict_pk.insert(&birth_id, &instance.#column_name)?;
                        }

                        dict_pk_by_pk.insert(&instance.#pk_name, &birth_id)?;

                        dict_index.insert(&birth_id, &instance.#pk_name)?;
                    };

                struct_initializer = quote! {
                    #column_name: {
                        let pk2birth = read_txn.open_table(#table_dict_pk_by_pk_ident)?;
                        let birth_guard = pk2birth.get(pk)?;
                        let birth_id = birth_guard.unwrap().value();
                        let birth2val = read_txn.open_table(#table_value_by_dict_pk_ident)?;
                        let val_guard = birth2val.get(&birth_id)?;
                        val_guard.unwrap().value()
                    }
                };

                let get_fn_name = format_ident!("get_by_{}", column_name);
                query_function = quote! {
                    pub fn #get_fn_name(
                        db: &::redb::Database,
                        val: &#column_type
                    ) -> Result<Vec<#struct_name>, DbEngineError> {
                        let read_txn = db.begin_read()?;

                        let val2birth = read_txn.open_table(#table_value_to_dict_pk_ident)?;
                        let birth_guard = val2birth.get(val)?;
                        let birth_id = match birth_guard {
                            Some(g) => g.value().clone(),
                            None => return Ok(Vec::new()),
                        };
                        let birth2pks = read_txn.open_multimap_table(#table_dict_index_ident)?;
                        let mut iter = birth2pks.get(&birth_id)?;
                        let mut results = Vec::new();
                        while let Some(x) = iter.next() {
                            let pk = x?.value();
                            match Self::compose(&read_txn, &pk) {
                                Ok(item) => {
                                    results.push(item);
                                }
                                Err(err) => {
                                    return Err(DbEngineError::DbError(err.to_string()));
                                }
                            }
                        }
                        Ok(results)
                    }
                }
            }
        }
        let column_macros =
            ColumnMacros {
                table_definitions,
                store_statement,
                struct_initializer,
                query_function,
            };

        columns.push((struct_column, column_macros));
    }
    println!("Tables for {}:\n{}", struct_name, table_names.join("\n"));
    StructMacros {
        struct_name,
        pk_name,
        pk_type,
        columns
    }
}

#[proc_macro_derive(Redbit, attributes(pk, column))]
pub fn derive_indexed(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;
    let named_fields = match get_named_fields(&ast) {
        Ok(columns) => columns,
        Err(err) => return err.to_compile_error().into(),
    };
    let (pk_ident, pk_type, struct_columns) =
        match parse_struct_columns(&named_fields, &ast.span()) {
            Ok(info) => info,
            Err(err) => return err.to_compile_error().into(),
        };

    let struct_macros = generate_struct_macros(struct_columns, struct_name.clone(), pk_ident, pk_type);

    TokenStream::from(struct_macros.expand())
}
