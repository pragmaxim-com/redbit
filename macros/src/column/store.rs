use crate::table::DictTableDefs;
use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn store_statement(pk_name: &Ident, column_name: &Ident, table_var: &Ident) -> TokenStream {
    quote! {
        tx_context.#table_var.insert(&instance.#pk_name, &instance.#column_name)?;
    }
}

pub fn store_many_statement(pk_name: &Ident, column_name: &Ident, table_var: &Ident) -> TokenStream {
    quote! {
        tx_context.#table_var.insert(&instance.#pk_name, &instance.#column_name)?;
    }
}

pub fn store_index_def(column_name: &Ident, pk_name: &Ident, table_var: &Ident, mm_var: &Ident) -> TokenStream {
    quote! {
        tx_context.#table_var.insert(&instance.#pk_name, &instance.#column_name)?;
        tx_context.#mm_var.insert(&instance.#column_name, &instance.#pk_name)?;
    }
}
pub fn store_many_index_def(column_name: &Ident, pk_name: &Ident, table_var: &Ident, mm_var: &Ident) -> TokenStream {
    quote! {
        tx_context.#table_var.insert(&instance.#pk_name, &instance.#column_name)?;
        tx_context.#mm_var.insert(&instance.#column_name, &instance.#pk_name)?;
    }
}

fn store_dict_stmnt(column_name: &Ident, pk_name: &Ident, dict_var: &Ident) -> TokenStream {
    quote! { tx_context.#dict_var.insert(instance.#pk_name, instance.#column_name)?; }
}

pub fn store_dict_def(column_name: &Ident, pk_name: &Ident, dict_table_defs: &DictTableDefs) -> TokenStream {
    let store_dict = store_dict_stmnt(column_name, pk_name, &dict_table_defs.var_name);
    quote! {
        #store_dict
    }
}
