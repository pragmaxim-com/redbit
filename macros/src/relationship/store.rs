use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;
use crate::table::StoreManyStmnt;

pub fn one2one_store_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_type::store(&tx, instance.#child_name)?;
    }
}

pub fn one2one_store_many_def(child_name: &Ident, child_type: &Type) -> StoreManyStmnt {
    let var_name_str = format!("{}s", child_name);
    let var_ident = Ident::new(&var_name_str, child_name.span());

    StoreManyStmnt {
        pre: quote! {
            let mut #var_ident = Vec::with_capacity(instances.len());
        },
        insert: quote! {
            #var_ident.push(instance.#child_name);
        },
        post: quote! {
            #child_type::store_many(&tx, #var_ident)?;
        }
    }
}

pub fn one2opt_store_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        if let Some(child) = instance.#child_name {
            #child_type::store(&tx, child)?;
        }
    }
}

pub fn one2opt_store_many_def(child_name: &Ident, child_type: &Type) -> StoreManyStmnt {
    let var_name_str = format!("{}s", child_name);
    let var_ident = Ident::new(&var_name_str, child_name.span());

    StoreManyStmnt {
        pre: quote! {
            let mut #var_ident = Vec::with_capacity(instances.len());
        },
        insert: quote! {
            if let Some(child) = instance.#child_name {
                #var_ident.push(child);
            }
        },
        post: quote! {
            #child_type::store_many(&tx, #var_ident)?;
        }
    }
}

pub fn one2many_store_def(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_type::store_many(&tx, instance.#child_name)?;
    }
}

pub fn one2many_store_many_def(child_name: &Ident, child_type: &Type) -> StoreManyStmnt {
    let var_name_str = format!("{}s", child_name);
    let var_ident = Ident::new(&var_name_str, child_name.span());

    StoreManyStmnt {
        pre: quote! {
            let mut #var_ident: Vec<#child_type> = Vec::new();
        },
        insert: quote! {
            #var_ident.append(&mut instance.#child_name);
        },
        post: quote! {
            #child_type::store_many(&tx, #var_ident)?;
        }
    }
}