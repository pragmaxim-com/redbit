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

pub fn one2many_store_def(child_name: &Ident, child_type: &Type, pk_name: &Ident, write_from: Option<WriteFrom>) -> TokenStream {
    let non_empty_children = quote! { #child_type::store_many(&tx_context.#child_name, instance.#child_name)?; };
    match write_from {
        Some(WriteFrom(write_from_field)) => {
            let hook_method_name = Ident::new(&format!("write_from_{}", write_from_field), child_name.span());
            quote! {
                if instance.#child_name.is_empty() {
                    crate::hook::#hook_method_name(&tx_context, instance.#pk_name, instance.#write_from_field)?;
                } else {
                    #non_empty_children
                }
            }
        },
        None => non_empty_children
    }
}