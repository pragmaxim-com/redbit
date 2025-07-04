use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn pk_init_expr() -> TokenStream {
    quote! {
        pk.clone()
    }
}

pub fn pk_init(pk_name: &Ident) -> TokenStream {
    let init_expr = pk_init_expr();
    quote! {
        #pk_name: #init_expr
    }
}

pub fn pk_init_with_query(pk_name: &Ident) -> TokenStream {
    let init_expr = pk_init_expr();
    quote! {
        let #pk_name = #init_expr;
    }
}

pub fn pk_default_init(pk_name: &Ident) -> TokenStream {
    let init_expr = pk_init_expr();
    quote! {
        #pk_name: #init_expr
    }
}
