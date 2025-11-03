use crate::entity::info::TableInfoItem;
use proc_macro2::Ident;
use quote::quote;
use syn::Type;

pub fn table_info_init(child_name: &Ident, child_table_info_type: &Type) -> TableInfoItem {
    let definition = quote! { pub #child_name: #child_table_info_type };
    let init = quote! { #child_name: #child_table_info_type::new_table_info(&tx_context.#child_name)? };
    TableInfoItem { definition, init }
}
