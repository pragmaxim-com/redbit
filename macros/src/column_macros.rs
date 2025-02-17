use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;

pub struct ColumnMacros {
    pub table_definitions: Vec<TokenStream>,
    pub store_statement: TokenStream,
    pub struct_initializer: TokenStream,
    pub query_function: Option<TokenStream>,
    pub range_function: Option<TokenStream>,
}

impl ColumnMacros {
    pub fn simple(struct_name: &Ident, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type) -> ColumnMacros {
        let mut table_definitions: Vec<TokenStream> = Vec::new();
        let mut table_names: Vec<String> = Vec::new();
        let table_ident = format_ident!(
            "{}_{}_BY_{}",
            struct_name.to_string().to_uppercase(),
            column_name.to_string().to_uppercase(),
            pk_name.to_string().to_uppercase()
        );
        let table_name_str = table_ident.to_string();
        table_names.push(table_name_str.clone());
        table_definitions.push(quote! {
            pub const #table_ident: ::redb::TableDefinition<'static, #pk_type, #column_type> = ::redb::TableDefinition::new(#table_name_str);
        });
        let store_statement = quote! {
            let mut table = write_txn.open_table(#table_ident)?;
            table.insert(&instance.#pk_name, &instance.#column_name)?;
        };
        let struct_initializer = quote! {
            #column_name: {
                let table = read_txn.open_table(#table_ident)?;
                let guard = table.get(pk)?;
                guard.unwrap().value()
            }
        };
        ColumnMacros { table_definitions, store_statement, struct_initializer, query_function: None, range_function: None }
    }

    pub fn indexed(struct_name: &Ident, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type, range: bool) -> ColumnMacros {
        let mut table_definitions: Vec<TokenStream> = Vec::new();
        let mut table_names: Vec<String> = Vec::new();

        let table_ident = format_ident!(
            "{}_{}_BY_{}",
            struct_name.to_string().to_uppercase(),
            column_name.to_string().to_uppercase(),
            pk_name.to_string().to_uppercase()
        );
        let table_name_str = table_ident.to_string();
        table_names.push(table_name_str.clone());
        table_definitions.push(quote! {
            pub const #table_ident: ::redb::TableDefinition<'static, #pk_type, #column_type> = ::redb::TableDefinition::new(#table_name_str);
        });

        let index_table_ident = format_ident!("{}_{}_INDEX", struct_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let index_table_name_str = index_table_ident.to_string();
        table_names.push(index_table_name_str.clone());
        table_definitions.push(quote! {
                    pub const #index_table_ident: ::redb::MultimapTableDefinition<'static, #column_type, #pk_type> = ::redb::MultimapTableDefinition::new(#index_table_name_str);
                });
        let store_statement = quote! {
            let mut table = write_txn.open_table(#table_ident)?;
            table.insert(&instance.#pk_name, &instance.#column_name)?;

            let mut mm = write_txn.open_multimap_table(#index_table_ident)?;
            mm.insert(&instance.#column_name, &instance.#pk_name)?;
        };
        let struct_initializer = quote! {
            #column_name: {
                let table = read_txn.open_table(#table_ident)?;
                let guard = table.get(pk)?;
                guard.unwrap().value()
            }
        };
        let get_fn_name = format_ident!("get_by_{}", column_name);
        let query_function = Some(quote! {
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
        });
        let range_function = if range {
            let range_fn_name = format_ident!("range_by_{}", column_name);
            Some(quote! {
                pub fn #range_fn_name(
                    db: &::redb::Database,
                    from: &#column_type,
                    to: &#column_type
                ) -> Result<Vec<#struct_name>, DbEngineError> {
                    let read_txn = db.begin_read()?;
                    let mm_table = read_txn.open_multimap_table(#index_table_ident)?;
                    let range_iter = mm_table.range(from.clone()..=to.clone())?;

                    let mut results = Vec::new();
                    for entry_res in range_iter {
                        let (col_key, mut multi_iter) = entry_res?;
                        while let Some(x) = multi_iter.next() {
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
                    }
                    Ok(results)
                }
            })
        } else {
            None
        };
        ColumnMacros { table_definitions, store_statement, struct_initializer, query_function, range_function }
    }

    pub fn indexed_with_dict(struct_name: &Ident, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type) -> ColumnMacros {
        let mut table_definitions: Vec<TokenStream> = Vec::new();
        let mut table_names: Vec<String> = Vec::new();

        let table_dict_pk_by_pk_ident = format_ident!(
            "{}_{}_DICT_PK_BY_{}",
            struct_name.to_string().to_uppercase(),
            column_name.to_string().to_uppercase(),
            pk_name.to_string().to_uppercase()
        );
        let table_dict_pk_by_pk_str = table_dict_pk_by_pk_ident.to_string();
        let table_value_to_dict_pk_ident =
            format_ident!("{}_{}_TO_DICT_PK", struct_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let table_value_to_dict_pk_str = table_value_to_dict_pk_ident.to_string();
        let table_value_by_dict_pk_ident =
            format_ident!("{}_{}_BY_DICT_PK", struct_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let table_value_by_dict_pk_str = table_value_by_dict_pk_ident.to_string();
        let table_dict_index_ident =
            format_ident!("{}_{}_DICT_INDEX", struct_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
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
        let store_statement = quote! {
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
        let struct_initializer = quote! {
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
        let query_function = Some(quote! {
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
        });
        ColumnMacros { table_definitions, store_statement, struct_initializer, query_function, range_function: None }
    }
}
