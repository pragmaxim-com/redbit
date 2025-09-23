use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn store_statement(pk_name: &Ident, column_name: &Ident, table_var: &Ident) -> TokenStream {
    quote! {
        tx_context.#table_var.insert_kv(instance.#pk_name, instance.#column_name)?;
    }
}

pub fn store_index_def(column_name: &Ident, pk_name: &Ident, index_table: &Ident) -> TokenStream {
    quote! {
        tx_context.#index_table.insert_kv(instance.#pk_name, instance.#column_name)?;
    }
}

fn store_dict_stmnt(column_name: &Ident, pk_name: &Ident, dict_table_var: &Ident) -> TokenStream {
    quote! { tx_context.#dict_table_var.insert_kv(instance.#pk_name, instance.#column_name)?; }
}

pub fn store_dict_def(column_name: &Ident, pk_name: &Ident, dict_table_var: &Ident) -> TokenStream {
    let store_dict = store_dict_stmnt(column_name, pk_name, dict_table_var);
    quote! {
        #store_dict
    }
}
