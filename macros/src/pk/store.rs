use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn store_statement(pk_name: &Ident, table: &Ident) -> TokenStream {
    quote! {
        let mut table_pk_1 = write_tx.open_table(#table)?;
        table_pk_1.insert(&instance.#pk_name, ())?;
    }
}

pub fn store_many_statement(pk_name: &Ident, table: &Ident) -> TokenStream {
    quote! {
        let mut table_pk_2 = write_tx.open_table(#table)?;
        for instance in instances.iter() {
            table_pk_2.insert(&instance.#pk_name, ())?;
        };
    }
}
