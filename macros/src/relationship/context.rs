use proc_macro2::Ident;
use quote::quote;
use syn::Type;
use crate::entity::context::TxContextItem;

pub fn tx_context_item(child_name: &Ident, child_tx_context_type: &Type) -> TxContextItem {
    let definition = quote! { pub #child_name: #child_tx_context_type<'txn> };
    let init = quote! { #child_name: #child_tx_context_type::begin_write_tx(tx)? };
    TxContextItem { definition, init }
}
