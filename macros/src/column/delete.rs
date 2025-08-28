use proc_macro2::{Ident, TokenStream};
use quote::quote;
use crate::column::open_dict_tables;
use crate::table::DictTableDefs;

pub fn delete_statement(table_var: &Ident) -> TokenStream {
    quote! {
        removed.push(tx_context.#table_var.remove(pk)?.is_some());
    }
}

pub fn delete_many_statement(table_var: &Ident) -> TokenStream {
    quote! {
        for pk in pks.iter() {
            if tx_context.#table_var.remove(pk)?.is_none() {
                removed.push(false);
            }
        }
    }
}

pub fn delete_index_statement(table_var: &Ident, index_table_var: &Ident) -> TokenStream {
    quote! {
        let maybe_value = {
            if let Some(value_guard) = tx_context.#table_var.remove(pk)? {
                Some(value_guard.value())
            } else {
                removed.push(false);
                None
            }
        };
        if let Some(value) = maybe_value {
            removed.push(tx_context.#index_table_var.remove(&value, pk)?);
        }
    }
}

pub fn delete_many_index_statement(table_var: &Ident, index_table_var: &Ident) -> TokenStream {
    quote! {
        for pk in pks.iter() {
            if let Some(value_guard) = tx_context.#table_var.remove(pk)? {
                let value = value_guard.value();
                removed.push(tx_context.#index_table_var.remove(&value, pk)?);
            } else {
                removed.push(false);
            }
        }
    }
}

fn cache_remove(cache_name: &Option<Ident>) -> TokenStream {
    match cache_name {
        Some(cache) => quote! {
           tx.cache_remove(&#cache, &value);
        },
        None => quote! {},
    }
}

pub fn delete_dict_statement(dict_table_defs: &DictTableDefs) -> TokenStream {
    let (dict_pk_by_pk_var, value_to_dict_pk_var, value_by_dict_pk_var, dict_index_var) =
        open_dict_tables(dict_table_defs);
    let cache_remove_stmnt = cache_remove(&dict_table_defs.value_to_dict_pk_cache);
    quote! {
        let birth_id_opt = tx_context.#dict_pk_by_pk_var.remove(pk)?.map(|guard| guard.value());
        if let Some(birth_id) = birth_id_opt {
            let value_opt = tx_context.#value_by_dict_pk_var.get(&birth_id)?.map(|guard| guard.value());
            if let Some(value) = value_opt {
                removed.push(tx_context.#dict_index_var.remove(&birth_id, pk)?);
                if tx_context.#dict_index_var.get(&birth_id)?.is_empty() {
                    tx_context.#value_to_dict_pk_var.remove(&value)?;
                    #cache_remove_stmnt
                    tx_context.#value_by_dict_pk_var.remove(&birth_id)?;
                }
            } else {
                removed.push(false);
            }
        } else {
            removed.push(false);
        }
    }
}

pub fn delete_many_dict_statement(dict_table_defs: &DictTableDefs) -> TokenStream {
    let (dict_pk_by_pk_var, value_to_dict_pk_var, value_by_dict_pk_var, dict_index_var) =
        open_dict_tables(dict_table_defs);
    let cache_remove_stmnt = cache_remove(&dict_table_defs.value_to_dict_pk_cache);

    quote! {
        for pk in pks.iter() {
            let birth_id_opt = tx_context.#dict_pk_by_pk_var.remove(pk)?.map(|guard| guard.value());
            if let Some(birth_id) = birth_id_opt { // duplicate
                let value_opt = tx_context.#value_by_dict_pk_var.get(&birth_id)?.map(|guard| guard.value());
                if let Some(value) = value_opt {
                    removed.push(tx_context.#dict_index_var.remove(&birth_id, pk)?);
                    if tx_context.#dict_index_var.get(&birth_id)?.is_empty() {
                        tx_context.#value_to_dict_pk_var.remove(&value)?;
                        #cache_remove_stmnt
                        tx_context.#value_by_dict_pk_var.remove(&birth_id)?;
                    }
                } else {
                    removed.push(false);
                }
            } else {
                removed.push(false);
            }
        }
    }
}
