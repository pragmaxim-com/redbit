use proc_macro2::Ident;
use quote::quote;
use syn::Type;
use crate::entity::context::TxContextItem;

pub fn tx_context_item(child_name: &Ident, write_child_tx_context_type: &Type, read_child_tx_context_type: &Type) -> TxContextItem {
    let write_definition = quote! { pub #child_name: #write_child_tx_context_type<'txn> };
    let write_init = quote! { #child_name: #write_child_tx_context_type::begin_write_tx(plain_tx, index_dbs)? };
    let write_flush = Some(quote! { self.#child_name.flush()? });
    let read_definition = quote! { pub #child_name: #read_child_tx_context_type };
    let read_init = quote! { #child_name: #read_child_tx_context_type::begin_read_tx(&storage)? };
    TxContextItem { write_definition, write_init, write_flush, read_definition, read_init}
}
