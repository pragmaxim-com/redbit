use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn delete_statement(table_var: &Ident) -> TokenStream {
    quote! {
        removed.push(tx_context.#table_var.router.delete_kv(pk)?);
    }
}

pub fn delete_index_statement(index_table_var: &Ident) -> TokenStream {
    quote! {
        removed.push(tx_context.#index_table_var.router.delete_kv(pk)?);
    }
}

pub fn delete_many_index_statement(index_table: &Ident) -> TokenStream {
    quote! {
        for pk in pks.iter() {
            removed.push(tx_context.#index_table.router.delete_kv(*pk)?);
        }
    }
}

pub fn delete_dict_statement(dict_table_var: &Ident) -> TokenStream {
    quote! {
        removed.push(tx_context.#dict_table_var.router.delete_kv(pk)?);
    }
}

pub fn delete_many_dict_statement(dict_table_var: &Ident) -> TokenStream {
    quote! {
        for pk in pks.iter() {
            removed.push(tx_context.#dict_table_var.router.delete_kv(*pk)?);
        }
    }
}
