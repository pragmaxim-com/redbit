use crate::entity::context::TxContextItem;
use crate::field_parser::WriteFrom;
use proc_macro2::Ident;
use quote::quote;
use syn::Type;

pub fn tx_context_item(child_name: &Ident, child_tx_context_type: &Type, write_child_tx_context_type: &Type, read_child_tx_context_type: &Type, write_from: Option<WriteFrom>) -> TxContextItem {
    let definition = quote! { pub #child_name: #child_tx_context_type };
    let def_constructor = quote! { #child_name: #child_tx_context_type::definition()? };
    let write_definition = quote! { pub #child_name: #write_child_tx_context_type };
    let write_constructor = quote! { #child_name: defs.#child_name.to_write_field(storage)? };
    let write_begin = quote! { self.#child_name.begin_writing_async(durability)? };
    let async_flush = if write_from.is_some() {
        Some(quote! { self.#child_name.commit_ctx_deferred()? })
    } else {
        Some(quote! { self.#child_name.commit_ctx_async()? })
    };
    let deferred_flush = Some(quote! { self.#child_name.commit_ctx_deferred()? });
    let write_shutdown = quote! { self.#child_name.stop_writing_async()? };
    let read_definition = quote! { pub #child_name: #read_child_tx_context_type };
    let read_constructor = quote! { #child_name: defs.#child_name.to_read_field(storage)? };
    TxContextItem {
        definition,
        def_constructor,
        write_definition,
        write_constructor,
        write_begin,
        async_flush,
        deferred_flush,
        write_shutdown,
        read_definition,
        read_constructor
    }
}
