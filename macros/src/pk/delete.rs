use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn delete_statement(table: &Ident) -> TokenStream {
    quote! {
        let mut table_pk_3 = write_tx.open_table(#table)?;
        let _ = table_pk_3.remove(pk)?;
    }
}

pub fn delete_many_statement(table: &Ident) -> TokenStream {
    quote! {
        let mut table_pk_4 = write_tx.open_table(#table)?;
        for pk in pks.iter() {
            table_pk_4.remove(pk)?;
        }
    }
}
