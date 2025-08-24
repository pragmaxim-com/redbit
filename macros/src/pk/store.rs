use proc_macro2::{Ident, TokenStream};
use quote::quote;
use crate::table::StoreManyStmnt;

pub fn store_statement(pk_name: &Ident, table: &Ident) -> TokenStream {
    let pk_var = Ident::new(&format!("{}_pk_var", table).to_lowercase(), table.span());
    quote! {
        let mut #pk_var = tx.open_table(#table)?;
        #pk_var.insert(&instance.#pk_name, ())?;
    }
}

pub fn store_many_statement(pk_name: &Ident, table: &Ident) -> StoreManyStmnt {
    let pk_var = Ident::new(&format!("{}_pk_var", table).to_lowercase(), table.span());
    StoreManyStmnt {
        pre: quote! {
            let mut #pk_var = tx.open_table(#table)?;
        },
        insert: quote! {
            #pk_var.insert(&instance.#pk_name, ())?;
        },
        post: quote!{}
    }
}
