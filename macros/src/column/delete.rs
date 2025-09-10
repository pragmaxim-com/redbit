use proc_macro2::{Ident, TokenStream};
use quote::quote;
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

pub fn delete_dict_statement(dict_table_defs: &DictTableDefs) -> TokenStream {
    let dict_table_var = &dict_table_defs.var_name;
    quote! {
        let deleted = tx_context.#dict_table_var.dict_delete(*pk)?;
        removed.push(deleted);
    }
}

pub fn delete_many_dict_statement(dict_table_defs: &DictTableDefs) -> TokenStream {
    let dict_table_var = &dict_table_defs.var_name;
    quote! {
        for pk in pks.iter() {
            let deleted = tx_context.#dict_table_var.dict_delete(*pk)?;
            removed.push(deleted);
        }
    }
}
