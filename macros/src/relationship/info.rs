use proc_macro2::Ident;
use quote::quote;
use syn::Type;
use crate::entity::info::TableInfoItem;

pub fn table_info_init(child_name: &Ident, child_table_info_type: &Type) -> TableInfoItem {
    let definition = quote! { pub #child_name: #child_table_info_type };
    let init = quote! { #child_name: #child_table_info_type::new_table_info(storage)? };
    TableInfoItem { definition, init }
}
