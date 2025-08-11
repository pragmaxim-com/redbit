use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn delete_statement(table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_3 = tx.open_table(#table)?;
        let table_col_3_old_value_opt = table_col_3.remove(pk)?;
        removed.push(table_col_3_old_value_opt.is_some());
    }
}

pub fn delete_many_statement(table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_4 = tx.open_table(#table)?;
        for pk in pks.iter() {
            if table_col_4.remove(pk)?.is_none() {
                removed.push(false);
            }
        }
    }
}

pub fn delete_index_statement(table: &Ident, index_table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_8 = tx.open_table(#table)?;
        let maybe_value = {
            if let Some(value_guard) = table_col_8.remove(pk)? {
                Some(value_guard.value().clone())
            } else {
                removed.push(false);
                None
            }
        };
        if let Some(value) = maybe_value {
            let mut mm = tx.open_multimap_table(#index_table)?;
            removed.push(mm.remove(&value, pk)?);
        }
    }
}

pub fn delete_many_index_statement(table: &Ident, index_table: &Ident) -> TokenStream {
    quote! {
        let mut table_col_9 = tx.open_table(#table)?;
        let mut mm = tx.open_multimap_table(#index_table)?;
        for pk in pks.iter() {
            if let Some(value_guard) = table_col_9.remove(pk)? {
                let value = value_guard.value();
                removed.push(mm.remove(&value, pk)?);
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

pub fn delete_dict_statement(table_dict_pk_by_pk: &Ident, table_value_to_dict_pk: &Ident, table_value_by_dict_pk: &Ident, table_dict_index: &Ident, table_value_to_dict_pk_cache: Option<Ident>) -> TokenStream {
    let cache_remove_stmnt = cache_remove(&table_value_to_dict_pk_cache);
    quote! {
        let mut dict_pk_by_pk       = tx.open_table(#table_dict_pk_by_pk)?;
        let mut value_to_dict_pk    = tx.open_table(#table_value_to_dict_pk)?;
        let mut value_by_dict_pk    = tx.open_table(#table_value_by_dict_pk)?;
        let mut dict_index          = tx.open_multimap_table(#table_dict_index)?;

        let birth_id_opt = dict_pk_by_pk.remove(pk)?.map(|guard| guard.value().clone());
        if let Some(birth_id) = birth_id_opt {
            let value_opt = value_by_dict_pk.get(&birth_id)?.map(|guard| guard.value().clone());
            if let Some(value) = value_opt {
                removed.push(dict_index.remove(&birth_id, pk)?);
                if dict_index.get(&birth_id)?.is_empty() {
                    value_to_dict_pk.remove(&value)?;
                    #cache_remove_stmnt
                    value_by_dict_pk.remove(&birth_id)?;
                }
            } else {
                removed.push(false);
            }
        } else {
            removed.push(false);
        }
    }
}

pub fn delete_many_dict_statement(table_dict_pk_by_pk: &Ident, table_value_to_dict_pk: &Ident, table_value_by_dict_pk: &Ident, table_dict_index: &Ident, table_value_to_dict_pk_cache: Option<Ident>) -> TokenStream {
    let cache_remove_stmnt = cache_remove(&table_value_to_dict_pk_cache);
    quote! {
        let mut dict_pk_by_pk       = tx.open_table(#table_dict_pk_by_pk)?;
        let mut value_to_dict_pk    = tx.open_table(#table_value_to_dict_pk)?;
        let mut value_by_dict_pk    = tx.open_table(#table_value_by_dict_pk)?;
        let mut dict_index          = tx.open_multimap_table(#table_dict_index)?;

        for pk in pks.iter() {
            let birth_id_opt = dict_pk_by_pk.remove(pk)?.map(|guard| guard.value().clone());
            if let Some(birth_id) = birth_id_opt { // duplicate
                let value_opt = value_by_dict_pk.get(&birth_id)?.map(|guard| guard.value().clone());
                if let Some(value) = value_opt {
                    removed.push(dict_index.remove(&birth_id, pk)?);
                    if dict_index.get(&birth_id)?.is_empty() {
                        value_to_dict_pk.remove(&value)?;
                        #cache_remove_stmnt
                        value_by_dict_pk.remove(&birth_id)?;
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
