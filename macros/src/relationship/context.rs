use proc_macro2::Ident;
use quote::quote;
use syn::Type;
use crate::entity::context::TxContextItem;

pub fn tx_context_item(child_name: &Ident, write_child_tx_context_type: &Type, read_child_tx_context_type: &Type) -> TxContextItem {
    let write_definition = quote! { pub #child_name: #write_child_tx_context_type };
    let write_constructor = quote! { #child_name: #write_child_tx_context_type::new_write_ctx(storage)? };
    let write_begin = quote! { let _ = &self.#child_name.begin_writing()? };
    let write_flush = Some(quote! { self.#child_name.commit_ctx_async()? });
    let write_shutdown = quote! { self.#child_name.stop_writing()? };
    let read_definition = quote! { pub #child_name: #read_child_tx_context_type };
    let read_constructor = quote! { #child_name: #read_child_tx_context_type::begin_read_ctx(storage)? };
    TxContextItem { write_definition, write_constructor, write_begin, write_flush, write_shutdown, read_definition, read_constructor }
}
