use crate::field_parser::{FieldDef, ReadFrom};
use crate::macro_utils;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

pub struct TransientRelationshipMacros {
    pub field_def: FieldDef,
    pub struct_init: TokenStream,
    pub struct_init_with_query: TokenStream,
    pub struct_default_init: TokenStream,
    pub struct_default_init_with_query: TokenStream,
}

impl TransientRelationshipMacros {
    pub fn read_from(field_name: &Ident, child_type: &Type, outer: Ident, inner: Ident) -> (TokenStream, TokenStream) {
        let inner_tx_context = macro_utils::one_to_many_field_name_from_type(child_type);
        (
            quote! {
                let #field_name = {
                    let mut result = Vec::with_capacity(#outer.len());
                    for in_field in &#outer {
                        if let Some(out_field) = #child_type::get(&tx_context.#inner_tx_context, in_field.#inner)? {
                            result.push(out_field);
                        }
                    }
                    result
                };
            },
            quote! {
                 let #field_name = {
                     #outer
                        .iter()
                        .take(3)
                        .enumerate()
                        .map(|(i, item)| #child_type::sample_with(item.#inner, i))
                        .collect::<Vec<_>>()
                };
            }
        )
    }

    pub fn new(field_def: FieldDef, read_from: Option<ReadFrom>) -> TransientRelationshipMacros {
        let child_name = &field_def.name; // e.g., "input_refs / input_utxos"
        let child_type = &field_def.tpe; // e.g., the type `InputRef` from Vec<InputRef>

        let (struct_init, default_init) = if let Some(ReadFrom { outer, inner }) = read_from {
            Self::read_from(child_name, child_type, outer, inner)
        } else {
            let default_init =
                quote! {
                    let #child_name = <#child_type as Sampleable>::sample_many(3);
                };
            (default_init.clone(), default_init)
        };

        TransientRelationshipMacros {
            field_def: field_def.clone(),
            struct_init: struct_init.clone(),
            struct_init_with_query: struct_init,
            struct_default_init: default_init.clone(),
            struct_default_init_with_query: default_init
        }
    }
}