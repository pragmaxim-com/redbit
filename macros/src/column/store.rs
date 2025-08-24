use proc_macro2::{Ident, TokenStream};
use quote::quote;
use crate::column::open_dict_tables;
use crate::table::DictTableDefs;

pub fn store_statement(pk_name: &Ident, column_name: &Ident, table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_1 = tx.open_table(#table)?;
        table_col_1.insert(&instance.#pk_name, &instance.#column_name)?;
    }
}

pub fn store_many_statement(pk_name: &Ident, column_name: &Ident, table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_2 = tx.open_table(#table)?;
        for instance in instances.iter() {
            table_col_2.insert(&instance.#pk_name, &instance.#column_name)?;
        }
    }
}

pub fn store_index_def(column_name: &Ident, pk_name: &Ident, table: &Ident, index_table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_6 = tx.open_table(#table)?;
        table_col_6.insert(&instance.#pk_name, &instance.#column_name)?;

        let mut mm_6 = tx.open_multimap_table(#index_table)?;
        mm_6.insert(&instance.#column_name, &instance.#pk_name)?;
    }
}

pub fn store_many_index_def(column_name: &Ident, pk_name: &Ident, table: &Ident, index_table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_7 = tx.open_table(#table)?;
        let mut mm_7 = tx.open_multimap_table(#index_table)?;
        for instance in instances.iter() {
            table_col_7.insert(&instance.#pk_name, &instance.#column_name)?;
            mm_7.insert(&instance.#column_name, &instance.#pk_name)?;
        };
    }
}

fn store_dict_stmnt(column_name: &Ident, pk_name: &Ident, cache: Option<Ident>) -> TokenStream {
    match cache {
        Some(cache_name) => quote! {
            let (birth_id, newly_created) =
                tx.cache_get_or_put(&#cache_name, instance.#column_name.clone(), || {
                    if let Some(guard) = value_to_dict_pk.get(&instance.#column_name)? {
                        Ok((guard.value(), false))
                    } else {
                        Ok((instance.#pk_name, true))
                    }
                })?;

            if newly_created {
                value_to_dict_pk.insert(&instance.#column_name, &birth_id)?;
                value_by_dict_pk.insert(&birth_id, &instance.#column_name)?;
            }

            dict_pk_by_pk.insert(&instance.#pk_name, &birth_id)?;
            dict_index.insert(&birth_id, &instance.#pk_name)?;
        },
        None => quote! {
            let (birth_id, newly_created) = {
                if let Some(guard) = value_to_dict_pk.get(&instance.#column_name)? {
                    (guard.value(), false)
                } else {
                    let new_birth = instance.#pk_name;
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
    }
}

pub fn store_dict_def(column_name: &Ident, pk_name: &Ident, dict_table_defs: &DictTableDefs) -> TokenStream {
    let opened_tables = open_dict_tables(dict_table_defs);
    let store_dict = store_dict_stmnt(column_name, pk_name, dict_table_defs.value_to_dict_pk_cache.clone());
    quote! {
        #opened_tables
        #store_dict
    }
}

pub fn store_many_dict_def(column_name: &Ident, pk_name: &Ident, dict_table_defs: &DictTableDefs) -> TokenStream {
    let opened_tables = open_dict_tables(dict_table_defs);
    let store_dict = store_dict_stmnt(column_name, pk_name, dict_table_defs.value_to_dict_pk_cache.clone());
    quote! {
        #opened_tables
        for instance in instances.iter() {
            #store_dict
        }
    }
}
