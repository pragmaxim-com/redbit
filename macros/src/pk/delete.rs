use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn delete_statement(table: &Ident) -> TokenStream {
    quote! {
        let mut table_pk_3 = tx.open_table(#table)?;
        let old_value_opt = table_pk_3.remove(pk)?;
        removed.push(old_value_opt.is_some());
    }
}

pub fn delete_many_statement(table: &Ident) -> TokenStream {
    quote! {
        let mut table_pk_4 = tx.open_table(#table)?;
        for pk in pks.iter() {
            if table_pk_4.remove(pk)?.is_none() {
                removed.push(false);
            }
        }
    }
}
