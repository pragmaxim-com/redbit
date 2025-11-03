use crate::entity::info::TableInfoItem;
use proc_macro2::Ident;
use quote::quote;

pub fn plain_table_info(column_name: &Ident, table_var: &Ident) -> TableInfoItem {
    let definition = quote! { pub #column_name: Vec<TableInfo> };
    let init = quote! { #column_name: tx_context.#table_var.stats()? };
    TableInfoItem { definition, init }
}

pub fn index_table_info(column_name: &Ident, index_table_var: &Ident) -> TableInfoItem {
    let definition = quote! { pub #column_name: Vec<TableInfo> };
    let init = quote! { #column_name: tx_context.#index_table_var.stats()? };
    TableInfoItem { definition, init }
}

pub fn dict_table_info(column_name: &Ident, dict_table_var: &Ident) -> TableInfoItem {
    let definition = quote! { pub #column_name: Vec<TableInfo> };
    let init = quote! { #column_name: tx_context.#dict_table_var.stats()? };
    TableInfoItem { definition, init }
}
