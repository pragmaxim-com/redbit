use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;
use crate::http_macros::*;

pub struct DbColumnMacros {
    pub table_definitions: Vec<(String, TokenStream)>,
    pub struct_init: TokenStream,
    pub struct_default_init: TokenStream,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_defs: Vec<FunctionDef>,
}

impl DbColumnMacros {
    pub fn plain(struct_name: &Ident, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type) -> DbColumnMacros {
        let mut table_definitions: Vec<(String, TokenStream)> = Vec::new();
        let table_ident = format_ident!(
            "{}_{}_BY_{}",
            struct_name.to_string().to_uppercase(),
            column_name.to_string().to_uppercase(),
            pk_name.to_string().to_uppercase()
        );
        let table_name_str = table_ident.to_string();
        table_definitions.push((
            table_name_str.clone(),
            quote! {
                pub const #table_ident: ::redb::TableDefinition<'static, Bincode<#pk_type>, Bincode<#column_type>> = ::redb::TableDefinition::new(#table_name_str);
            }
        ));
        let store_statement = quote! {
            let mut table_col_1 = write_tx.open_table(#table_ident)?;
            table_col_1.insert(&instance.#pk_name, &instance.#column_name)?;
        };
        let store_many_statement = quote! {
            let mut table_col_2 = write_tx.open_table(#table_ident)?;
            for instance in instances.iter() {
                table_col_2.insert(&instance.#pk_name, &instance.#column_name)?;
            }
        };
        let delete_statement = quote! {
            let mut table_col_3 = write_tx.open_table(#table_ident)?;
            let _ = table_col_3.remove(pk)?;
        };
        let delete_many_statement = quote! {
            let mut table_col_4 = write_tx.open_table(#table_ident)?;
            for pk in pks.iter() {
                table_col_4.remove(pk)?;
            }
        };
        let struct_init = quote! {
            #column_name: {
                let table_col_5 = read_tx.open_table(#table_ident)?;
                let guard = table_col_5.get(pk)?;
                guard.ok_or_else(|| AppError::NotFound(format!(
                        "table `{}`: no row for primary key {:?}",
                        stringify!(#table_ident),
                        pk
                    ))
                )?.value()
            }
        };
        let struct_default_init = quote! {
            #column_name: #column_type::default()
        };
        DbColumnMacros {
            table_definitions,
            struct_init,
            struct_default_init,
            store_statement,
            store_many_statement,
            delete_statement,
            delete_many_statement,
            function_defs: vec![]
        }
    }

    pub fn indexed(struct_name: &Ident, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type, range: bool) -> DbColumnMacros {
        let mut table_definitions: Vec<(String, TokenStream)> = Vec::new();

        let table_ident = format_ident!(
            "{}_{}_BY_{}",
            struct_name.to_string().to_uppercase(),
            column_name.to_string().to_uppercase(),
            pk_name.to_string().to_uppercase()
        );
        let table_name_str = table_ident.to_string();
        table_definitions.push((
            table_name_str.clone(),
            quote! {
                pub const #table_ident: ::redb::TableDefinition<'static, Bincode<#pk_type>, Bincode<#column_type>> = ::redb::TableDefinition::new(#table_name_str);
            }
        ));

        let index_table_ident = format_ident!("{}_{}_INDEX", struct_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let index_table_name_str = index_table_ident.to_string();
        table_definitions.push((index_table_name_str.clone(), quote! {
                    pub const #index_table_ident: ::redb::MultimapTableDefinition<'static, Bincode<#column_type>, Bincode<#pk_type>> = ::redb::MultimapTableDefinition::new(#index_table_name_str);
                }));
        let store_statement = quote! {
            let mut table_col_6 = write_tx.open_table(#table_ident)?;
            table_col_6.insert(&instance.#pk_name, &instance.#column_name)?;

            let mut mm = write_tx.open_multimap_table(#index_table_ident)?;
            mm.insert(&instance.#column_name, &instance.#pk_name)?;
        };
        let store_many_statement = quote! {
            let mut table_col_7 = write_tx.open_table(#table_ident)?;
            let mut mm = write_tx.open_multimap_table(#index_table_ident)?;
            for instance in instances.iter() {
                table_col_7.insert(&instance.#pk_name, &instance.#column_name)?;
                mm.insert(&instance.#column_name, &instance.#pk_name)?;
            };
        };
        let delete_statement = quote! {
            {
                let mut table_col_8 = write_tx.open_table(#table_ident)?;
                let maybe_value = {
                    if let Some(value_guard) = table_col_8.remove(pk)? {
                        Some(value_guard.value().clone())
                    } else {
                        None
                    }
                };
                if let Some(value) = maybe_value {
                    let mut mm = write_tx.open_multimap_table(#index_table_ident)?;
                    mm.remove(&value, pk)?;
                }
            }
        };
        let delete_many_statement = quote! {
            let mut table_col_9 = write_tx.open_table(#table_ident)?;
            let mut mm = write_tx.open_multimap_table(#index_table_ident)?;
            for pk in pks.iter() {
                if let Some(value_guard) = table_col_9.remove(pk)? {
                    let value = value_guard.value();
                    mm.remove(&value, pk)?;
                }
            }
        };
        let struct_init = quote! {
            #column_name: {
                let table_col_10 = read_tx.open_table(#table_ident)?;
                let guard = table_col_10.get(pk)?;
                guard.ok_or_else(|| AppError::NotFound(format!(
                        "table `{}`: no row for primary key {:?}",
                        stringify!(#table_ident),
                        pk
                    ))
                )?.value()
            }
        };
        let struct_default_init = quote! {
            #column_name: #column_type::default()
        };
        let mut function_defs: Vec<FunctionDef> = Vec::new();
        let get_by_name = format_ident!("get_by_{}", column_name);
        let get_by_fn =  quote! {
            pub fn #get_by_name(
                read_tx: &::redb::ReadTransaction,
                val: &#column_type
            ) -> Result<Vec<#struct_name>, AppError> {
                let mm_table = read_tx.open_multimap_table(#index_table_ident)?;
                let mut iter = mm_table.get(val)?;
                let mut results = Vec::new();
                while let Some(x) = iter.next() {
                    let pk = x?.value();
                    match Self::compose(&read_tx, &pk) {
                        Ok(item) => {
                            results.push(item);
                        }
                        Err(err) => {
                            return Err(AppError::Internal(err.to_string()));
                        }
                    }
                }
                Ok(results)
            }
        };
        function_defs.push(FunctionDef {
            entity: struct_name.clone(),
            name: get_by_name.clone(),
            stream: get_by_fn,
            return_value: ReturnValue{ value_name: struct_name.clone(), value_type: syn::parse_quote!(Vec<#struct_name>) },
            endpoint: Some(Endpoint::GetBy(Params { column_name: column_name.clone(), column_type: column_type.clone()})),
        });

        if range {
            let range_by_name = format_ident!("range_by_{}", column_name);
            let range_by_fn = quote! {
                pub fn #range_by_name(
                    read_tx: &::redb::ReadTransaction,
                    from: &#column_type,
                    until: &#column_type
                ) -> Result<Vec<#struct_name>, AppError> {
                    let mm_table = read_tx.open_multimap_table(#index_table_ident)?;
                    let range_iter = mm_table.range(from.clone()..until.clone())?;
                    let mut results = Vec::new();
                    for entry_res in range_iter {
                        let (col_key, mut multi_iter) = entry_res?;
                        while let Some(x) = multi_iter.next() {
                            let pk = x?.value();
                            match Self::compose(&read_tx, &pk) {
                                Ok(item) => {
                                    results.push(item);
                                }
                                Err(err) => {
                                    return Err(AppError::Internal(err.to_string()));
                                }
                            }
                        }
                    }
                    Ok(results)
                }
            };
            function_defs.push(FunctionDef {
                entity: struct_name.clone(),
                name: range_by_name.clone(),
                stream: range_by_fn,
                return_value: ReturnValue{ value_name: struct_name.clone(), value_type: syn::parse_quote!(Vec<#struct_name>) },
                endpoint: Some(Endpoint::RangeBy(Params { column_name: column_name.clone(), column_type: column_type.clone()})),
            });
        };
        DbColumnMacros {
            table_definitions,
            struct_init,
            struct_default_init,
            store_statement,
            store_many_statement,
            delete_statement,
            delete_many_statement,
            function_defs,
        }
    }

    pub fn indexed_with_dict(struct_name: &Ident, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type) -> DbColumnMacros {
        let mut table_definitions: Vec<(String, TokenStream)> = Vec::new();

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

        table_definitions.push((
            table_dict_pk_by_pk_str.clone(),
            quote! {
                pub const #table_dict_pk_by_pk_ident: ::redb::TableDefinition<'static, Bincode<#pk_type>, Bincode<#pk_type>> = ::redb::TableDefinition::new(#table_dict_pk_by_pk_str);
            }
        ));
        table_definitions.push((
            table_value_to_dict_pk_str.clone(),
            quote! {
                pub const #table_value_to_dict_pk_ident: ::redb::TableDefinition<'static, Bincode<#column_type>, Bincode<#pk_type>> = ::redb::TableDefinition::new(#table_value_to_dict_pk_str);
            }
        ));
        table_definitions.push((
            table_value_by_dict_pk_str.clone(),
            quote! {
                pub const #table_value_by_dict_pk_ident: ::redb::TableDefinition<'static, Bincode<#pk_type>, Bincode<#column_type>> = ::redb::TableDefinition::new(#table_value_by_dict_pk_str);
            }
        ));
        table_definitions.push((
            table_dict_index_str.clone(),
            quote! {
                pub const #table_dict_index_ident: ::redb::MultimapTableDefinition<'static, Bincode<#pk_type>, Bincode<#pk_type>>= ::redb::MultimapTableDefinition::new(#table_dict_index_str);
            }
        ));
        let store_statement = quote! {
            let mut dict_pk_by_pk       = write_tx.open_table(#table_dict_pk_by_pk_ident)?;
            let mut value_to_dict_pk    = write_tx.open_table(#table_value_to_dict_pk_ident)?;
            let mut value_by_dict_pk    = write_tx.open_table(#table_value_by_dict_pk_ident)?;
            let mut dict_index          = write_tx.open_multimap_table(#table_dict_index_ident)?;

            let (birth_id, newly_created) = {
                if let Some(guard) = value_to_dict_pk.get(&instance.#column_name)? {
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
        let store_many_statement = quote! {
            let mut dict_pk_by_pk       = write_tx.open_table(#table_dict_pk_by_pk_ident)?;
            let mut value_to_dict_pk    = write_tx.open_table(#table_value_to_dict_pk_ident)?;
            let mut value_by_dict_pk    = write_tx.open_table(#table_value_by_dict_pk_ident)?;
            let mut dict_index          = write_tx.open_multimap_table(#table_dict_index_ident)?;

            for instance in instances.iter() {
                 let (birth_id, newly_created) = {
                    if let Some(guard) = value_to_dict_pk.get(&instance.#column_name)? {
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
            }
        };
        let delete_statement = quote! {
            let mut dict_pk_by_pk       = write_tx.open_table(#table_dict_pk_by_pk_ident)?;
            let mut value_to_dict_pk    = write_tx.open_table(#table_value_to_dict_pk_ident)?;
            let mut value_by_dict_pk    = write_tx.open_table(#table_value_by_dict_pk_ident)?;
            let mut dict_index          = write_tx.open_multimap_table(#table_dict_index_ident)?;

            let birth_id_opt = dict_pk_by_pk.remove(pk)?.map(|guard| guard.value().clone());
            if let Some(birth_id) = birth_id_opt {
                let value_opt = value_by_dict_pk.get(&birth_id)?.map(|guard| guard.value().clone());
                if let Some(value) = value_opt {
                    dict_index.remove(&birth_id, pk)?;
                    if dict_index.get(&birth_id)?.is_empty() {
                        value_to_dict_pk.remove(&value)?;
                        value_by_dict_pk.remove(&birth_id)?;
                    }
                }
            }
        };
        let delete_many_statement = quote! {
            let mut dict_pk_by_pk       = write_tx.open_table(#table_dict_pk_by_pk_ident)?;
            let mut value_to_dict_pk    = write_tx.open_table(#table_value_to_dict_pk_ident)?;
            let mut value_by_dict_pk    = write_tx.open_table(#table_value_by_dict_pk_ident)?;
            let mut dict_index          = write_tx.open_multimap_table(#table_dict_index_ident)?;

            for pk in pks.iter() {
                let birth_id_opt = dict_pk_by_pk.remove(pk)?.map(|guard| guard.value().clone());
                if let Some(birth_id) = birth_id_opt { // duplicate
                    let value_opt = value_by_dict_pk.get(&birth_id)?.map(|guard| guard.value().clone());
                    if let Some(value) = value_opt {
                        dict_index.remove(&birth_id, pk)?;
                        if dict_index.get(&birth_id)?.is_empty() {
                            value_to_dict_pk.remove(&value)?;
                            value_by_dict_pk.remove(&birth_id)?;
                        }
                    }
                }
            }
        };
        let struct_init = quote! {
            #column_name: {
                let pk2birth = read_tx.open_table(#table_dict_pk_by_pk_ident)?;
                let birth_guard = pk2birth.get(pk)?;
                let birth_id = birth_guard.ok_or_else(|| AppError::NotFound(format!(
                        "table_dict_pk_by_pk_ident `{}`: no row for primary key {:?}",
                        stringify!(#table_dict_pk_by_pk_ident),
                        pk
                    ))
                )?.value();
                let birth2val = read_tx.open_table(#table_value_by_dict_pk_ident)?;
                let val_guard = birth2val.get(&birth_id)?;
                val_guard.ok_or_else(|| AppError::NotFound(format!(
                        "table_value_by_dict_pk_ident `{}`: no row for birth id {:?}",
                        stringify!(#table_value_by_dict_pk_ident),
                        birth_id
                    ))
                )?.value()
            }
        };
        let struct_default_init = quote! {
            #column_name: #column_type::default()
        };
        let mut function_defs: Vec<FunctionDef> = Vec::new();
        let get_by_name = format_ident!("get_by_{}", column_name);
        let get_by_fn = quote! {
            pub fn #get_by_name(
                read_tx: &::redb::ReadTransaction,
                val: &#column_type
            ) -> Result<Vec<#struct_name>, AppError> {
                let val2birth = read_tx.open_table(#table_value_to_dict_pk_ident)?;
                let birth_guard = val2birth.get(val)?;
                let birth_id = match birth_guard {
                    Some(g) => g.value().clone(),
                    None => return Ok(Vec::new()),
                };
                let birth2pks = read_tx.open_multimap_table(#table_dict_index_ident)?;
                let mut iter = birth2pks.get(&birth_id)?;
                let mut results = Vec::new();
                while let Some(x) = iter.next() {
                    let pk = x?.value();
                    match Self::compose(&read_tx, &pk) {
                        Ok(item) => {
                            results.push(item);
                        }
                        Err(err) => {
                            return Err(AppError::Internal(err.to_string()));
                        }
                    }
                }
                Ok(results)
            }
        };
        function_defs.push(FunctionDef {
            entity: struct_name.clone(),
            name: get_by_name.clone(),
            stream: get_by_fn,
            return_value: ReturnValue{ value_name: struct_name.clone(), value_type: syn::parse_quote!(Vec<#struct_name>) },
            endpoint: Some(Endpoint::GetBy(Params { column_name: column_name.clone(), column_type: column_type.clone()})),
        });
        DbColumnMacros {
            table_definitions,
            struct_init,
            struct_default_init,
            store_statement,
            store_many_statement,
            delete_statement,
            delete_many_statement,
            function_defs
        }
    }
}
