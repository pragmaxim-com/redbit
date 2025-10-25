use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;
use crate::field_parser::WriteFrom;
use crate::relationship::WriteFromStatement;

pub fn one2one_store_def(child_name: &Ident, child_type: &Type) -> TokenStream {
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

pub fn one2many_store_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! { #child_type::store_many(&tx_context.#child_name, instance.#child_name, is_last)?; }
}

pub fn one2many_write_from_def(entity_name: &Ident, child_name: &Ident, pk_name: &Ident, write_from_using: WriteFrom) -> WriteFromStatement {
    let WriteFrom { from, using } = write_from_using;
    let hook_method_name = Ident::new(&format!("write_from_{}_using_{}", from, using), child_name.span());
    let init_instances = Ident::new(&format!("{}_instances", entity_name.to_string().to_lowercase()), entity_name.span());
    let init = quote! { let mut #init_instances = IndexMap::with_capacity(instances.iter().map(|i| i.#from.len()).sum()); };
    let collect = quote! {
        for from in instance.#from {
            #init_instances.insert(from, instance.#pk_name);
        }
    };
    let store =
        quote! {
            crate::hook::#hook_method_name(&tx_context, #init_instances, is_last)?;
        };
    WriteFromStatement {
        init,
        collect,
        store,
    }
}