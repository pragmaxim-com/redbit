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

fn write_from_init(from: &Ident, many: bool) -> TokenStream {
    let init_instances = Ident::new(&format!("{}_instances", from), from.span());
    if many {
        quote! { let mut #init_instances = indexmap::IndexMap::with_capacity(instances.iter().map(|i| i.#from.len()).sum()); }
    } else {
        quote! { let mut #init_instances = indexmap::IndexMap::with_capacity(instance.#from.len()); }
    }
}

pub fn one2many_write_from_def(child_name: &Ident, pk_name: &Ident, write_from_using: WriteFrom, many: bool) -> WriteFromStatement {
    let WriteFrom { from, using } = write_from_using;
    let hook_method_name = Ident::new(&format!("write_from_{}_using_{}", from, using), child_name.span());
    let init_instances = Ident::new(&format!("{}_instances", from), from.span());
    let init = write_from_init(&from, many);
    let collect = quote! {
        for (idx, from) in instance.#from.into_iter().enumerate() {
            match #init_instances.entry(from) {
                indexmap::map::Entry::Vacant(v) => { v.insert((instance.#pk_name, idx)); }
                indexmap::map::Entry::Occupied(v) => {
                    return Err(AppError::Custom(format!("Double spend not supported {:?}", v)))
                },
            }
        }
    };
    let store =
        quote! {
            let entries = #init_instances.iter().map(|(k, &v)| (k.clone(), v)).collect();
            crate::hook::#hook_method_name(&tx_context, entries, is_last)?;
        };
    WriteFromStatement {
        init,
        collect,
        store,
    }
}
