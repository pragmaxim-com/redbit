use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn pk_init(pk_name: &Ident) -> TokenStream {
    quote! {
        let #pk_name = pk.clone();
    }
}
