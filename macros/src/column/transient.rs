use crate::field_parser::FieldDef;
use proc_macro2::TokenStream;
use quote::quote;

pub struct TransientMacros {
    pub field_def: FieldDef,
    pub struct_init: TokenStream,
    pub struct_init_with_query: TokenStream,
    pub struct_default_init: TokenStream,
    pub struct_default_init_with_query: TokenStream,
}

impl TransientMacros {
    pub fn new(field_def: FieldDef) -> TransientMacros {
        let field_name = &field_def.name;
        let field_type = &field_def.tpe;

        let default_init = quote! { let #field_name = <#field_type>::default(); };

        TransientMacros {
            field_def: field_def.clone(),
            struct_init: default_init.clone(),
            struct_init_with_query: default_init.clone(),
            struct_default_init: default_init.clone(),
            struct_default_init_with_query: default_init
        }
    }
}
