use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;
use crate::field_parser::WriteFrom;

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
        let #child_name = #child_type::sample_with(pk, sample_index);
    }
}

pub fn one2one_relation_default_init_with_query(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = {
            if let Some(child_query) = stream_query.#child_name.clone() {
                if let Some(child) = #child_type::sample_with_query(pk, sample_index, &child_query) {
                    child
                } else {
                    return None; // short-circuit
                }
            } else {
                #child_type::sample_with(pk, sample_index)
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
        let #child_name = Some(#child_type::sample_with(pk, sample_index));
    }
}

pub fn one2opt_relation_default_init_with_query(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = {
            if let Some(child_query) = stream_query.#child_name.clone() {
                #child_type::sample_with_query(pk, sample_index, &child_query)
            } else {
                Some(#child_type::sample_with(pk, sample_index))
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

pub fn one2many_relation_default_init(child_name: &Ident, child_type: &Type, write_from: Option<WriteFrom>) -> TokenStream {
    let wf = if write_from.is_some() { true } else { false };
    quote! {
        let #child_name = {
            let (from, _) = pk.fk_range();
            let idx = if #wf { 0 } else { sample_index };
            let sample_0 = #child_type::sample_with(from, idx);
            let sample_1 = #child_type::sample_with(from.next_index(), idx + 1);
            let sample_2 = #child_type::sample_with(from.next_index().next_index(), idx + 2);
            vec![sample_0, sample_1, sample_2]
        };
    }
}

pub fn one2many_relation_default_init_with_query(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        let #child_name = {
            let (from, _) = pk.fk_range();
            let sample_0 =
                if let Some(child_query) = stream_query.#child_name.clone() {
                    #child_type::sample_with_query(from, 0, &child_query)
                } else {
                    Some(#child_type::sample_with(from, 0))
                };
            let sample_1 =
                if let Some(child_query) = stream_query.#child_name.clone() {
                    #child_type::sample_with_query(from.next_index(), 1, &child_query)
                } else {
                    Some(#child_type::sample_with(from.next_index(), 1))
                };
            let sample_2 =
                if let Some(child_query) = stream_query.#child_name.clone() {
                    #child_type::sample_with_query(from.next_index().next_index(), 2, &child_query)
                } else {
                    Some(#child_type::sample_with(from.next_index().next_index(), 2))
                };

            vec![sample_0, sample_1, sample_2].into_iter().flatten().collect::<Vec<#child_type>>()
        };
    }
}