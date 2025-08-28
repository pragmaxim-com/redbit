use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn store_statement(pk_name: &Ident, table: &Ident) -> TokenStream {
    let table_var = Ident::new(&format!("{}", table).to_lowercase(), table.span());
    quote! {
        tx_context.#table_var.insert(&instance.#pk_name, ())?;
    }
}

pub fn store_many_statement(pk_name: &Ident, table: &Ident) -> TokenStream {
    let table_var = Ident::new(&format!("{}", table).to_lowercase(), table.span());
    quote! {
        tx_context.#table_var.insert(&instance.#pk_name, ())?;
    }
}
