use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

pub fn one2one_relation_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = #child_type::get(&tx_context.#child_name, pk)?.ok_or_else(|| AppError::NotFound(format!("Missing one-to-one child {:?}", pk)))?;
    }
}

pub fn one2one_relation_init_with_query(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = {
            if let Some(ref q) = stream_query.#child_name {
                let result = #child_type::filter(&tx_context.#child_name, pk, q)?;
                if result.is_none() {
                    return Ok(None); // short-circuit
                }
                result.unwrap()
            } else {
                #child_type::get(&tx_context.#child_name, pk)?.ok_or_else(|| AppError::NotFound(format!("Missing one-to-one child {:?}", pk)))?
            }
        };
    }
}

pub fn one2one_relation_default_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = #child_type::sample_with(pk);
    }
}

pub fn one2one_relation_default_init_with_query(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = {
            if let Some(child_query) = stream_query.#child_name.clone() {
                if let Some(child) = #child_type::sample_with_query(pk, &child_query) {
                    child
                } else {
                    return None; // short-circuit
                }
            } else {
                #child_type::sample_with(pk)
            }
        };
    }
}

pub fn one2opt_relation_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = #child_type::get(&tx_context.#child_name, pk)?;
    }
}

pub fn one2opt_relation_init_with_query(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = {
            if let Some(ref q) = stream_query.#child_name {
                let result = #child_type::filter(&tx_context.#child_name, pk, q)?;
                if result.is_none() {
                    return Ok(None); // short-circuit
                }
                result
            } else {
                #child_type::get(&tx_context.#child_name, pk)?
            }
        };
    }
}

pub fn one2opt_relation_default_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = Some(#child_type::sample_with(pk));
    }
}

pub fn one2opt_relation_default_init_with_query(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = {
            if let Some(child_query) = stream_query.#child_name.clone() {
                #child_type::sample_with_query(pk, &child_query)
            } else {
                Some(#child_type::sample_with(pk))
            }
        };
    }
}

pub fn one2many_relation_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = {
            let (from, to) = pk.fk_range();
            #child_type::range(&tx_context.#child_name, from, to, None)?
        };
    }
}

pub fn one2many_relation_init_with_query(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = {
            let (from, to) = pk.fk_range();
            let children = #child_type::range(&tx_context.#child_name, from, to, stream_query.#child_name.clone())?;
            if children.is_empty() {
                return Ok(None); // short-circuit
            }
            children
        };
    }
}

pub fn one2many_relation_default_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = {
            let (from, _) = pk.fk_range();
            #child_type::sample_many(from, 3)
        };
    }
}

pub fn one2many_relation_default_init_with_query(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = {
            let (from, _) = pk.fk_range();
            if let Some(child_query) = stream_query.#child_name.clone() {
                #child_type::sample_many_with_query(from, &child_query, 3)
            } else {
                #child_type::sample_many(from, 3)
            }
        };
    }
}