use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn store_statement(pk_name: &Ident, table_var: &Ident) -> TokenStream {
    quote! {
        tx_context.#table_var.insert_kv(instance.#pk_name, ())?;
    }
}

pub fn store_many_statement(pk_name: &Ident, table_var: &Ident) -> TokenStream {
    quote! {
        tx_context.#table_var.insert_kv(instance.#pk_name, ())?;
    }
}
