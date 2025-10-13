use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;
use crate::field_parser::WriteFrom;

pub fn one2one_store_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_type::store(&tx_context.#child_name, instance.#child_name)?;
    }
}

pub fn one2one_store_many_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_type::store(&tx_context.#child_name, instance.#child_name)?;
    }
}

pub fn one2opt_store_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        if let Some(child) = instance.#child_name {
            #child_type::store(&tx_context.#child_name, child)?;
        }
    }
}

pub fn one2opt_store_many_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        if let Some(child) = instance.#child_name {
            #child_type::store(&tx_context.#child_name, child)?;
        }
    }
}

pub fn one2many_store_def(child_name: &Ident, child_type: &Type, pk_name: &Ident, write_from_using: Option<WriteFrom>) -> TokenStream {
    let non_empty_children = quote! { #child_type::store_many(&tx_context.#child_name, instance.#child_name, is_last)?; };
    match write_from_using {
        Some(WriteFrom { from, using }) => {
            let hook_method_name = Ident::new(&format!("write_from_{}_using_{}", from, using), child_name.span());
            quote! {
                if !instance.#child_name.is_empty() {
                    #non_empty_children
                    if is_last {
                        crate::hook::flush(&tx_context)?;
                    }
                } else {
                    crate::hook::#hook_method_name(&tx_context, instance.#pk_name, instance.#from, is_last)?;
                }
            }
        },
        None => non_empty_children
    }
}