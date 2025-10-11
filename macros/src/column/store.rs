use proc_macro2::{Ident, TokenStream};
use quote::{quote, format_ident};
use crate::field_parser::UsedBy;

#[inline]
fn insert_method_ident(used_by: &Option<UsedBy>) -> Ident {
    if used_by.is_some() { format_ident!("insert_now") } else { format_ident!("insert_on_flush") }
}

pub fn store_statement(pk_name: &Ident, column_name: &Ident, table_var: &Ident, used_by: Option<UsedBy>) -> TokenStream {
    let method = insert_method_ident(&used_by);
    quote! {
        tx_context.#table_var.#method(instance.#pk_name, instance.#column_name)?;
    }
}

pub fn store_index_def(column_name: &Ident, pk_name: &Ident, index_table: &Ident, used_by: Option<UsedBy>) -> TokenStream {
    let method = insert_method_ident(&used_by);
    quote! {
        tx_context.#index_table.#method(instance.#pk_name, instance.#column_name)?;
    }
}

pub fn store_dict_def(column_name: &Ident, pk_name: &Ident, dict_table_var: &Ident, used_by: Option<UsedBy>) -> TokenStream {
    let method = insert_method_ident(&used_by);
    quote! {
        tx_context.#dict_table_var.#method(instance.#pk_name, instance.#column_name)?;
    }
}
