use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

pub fn one2one_store_def(child_name: &Ident, child_type: &Type) -> TokenStream { 
    quote! {
        let child = &instance.#child_name;
        #child_type::store(&tx, child)?;
    }
}

pub fn one2one_store_many_def(child_name: &Ident, child_type: &Type) -> TokenStream { 
    quote! {
        let children = instances.iter().map(|instance| instance.#child_name.clone()).collect();
        #child_type::store_many(&tx, &children)?;
    }
}

pub fn one2option_store_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        if let Some(child) = &instance.#child_name {
            #child_type::store(&tx, child)?;
        }
    }
}

pub fn one2option_store_many_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let mut children = Vec::with_capacity(instances.len());
        for instance in instances {
            if let Some(child) = &instance.#child_name {
                children.push(child.clone());
            }
        }
        #child_type::store_many(&tx, &children)?;
    }
}

pub fn one2many_store_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_type::store_many(&tx, &instance.#child_name)?;
    }
}

pub fn one2many_store_many_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let mut children: Vec<#child_type> = Vec::new();
        for instance in instances.iter() {
            children.extend_from_slice(&instance.#child_name)
        };
        #child_type::store_many(&tx, &children)?;
    }
}
