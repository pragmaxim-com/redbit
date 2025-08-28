use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

pub fn one2one_store_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_type::store(&mut tx_context.#child_name, instance.#child_name)?;
    }
}

pub fn one2one_store_many_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_type::store(&mut tx_context.#child_name, instance.#child_name)?;
    }
}

pub fn one2opt_store_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        if let Some(child) = instance.#child_name {
            #child_type::store(&mut tx_context.#child_name, child)?;
        }
    }
}

pub fn one2opt_store_many_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        if let Some(child) = instance.#child_name {
            #child_type::store(&mut tx_context.#child_name, child)?;
        }
    }
}

pub fn one2many_store_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_type::store_many(&mut tx_context.#child_name, instance.#child_name)?;
    }
}

pub fn one2many_store_many_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_type::store_many(&mut tx_context.#child_name, instance.#child_name)?;
    }
}