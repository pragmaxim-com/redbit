use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn delete_statement(table_var: &Ident) -> TokenStream {
    quote! {
        removed.push(tx_context.#table_var.remove(pk)?.is_some());
    }
}

pub fn delete_many_statement(table_var: &Ident) -> TokenStream {
    quote! {
        for pk in pks.iter() {
            if tx_context.#table_var.remove(pk)?.is_none() {
                removed.push(false);
            }
        }
    }
}
