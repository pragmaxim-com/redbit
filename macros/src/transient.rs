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
        let struct_default_init = quote! {
            #field_name: (<#field_type>::default())
        };
        let struct_init_with_query = quote! {
            let #field_name = (<#field_type>::default());
        };
        let struct_default_init_with_query = quote! {
            let #field_name = (<#field_type>::default());
        };
        TransientMacros {
            field_def,
            struct_init: struct_default_init.clone(),
            struct_init_with_query,
            struct_default_init,
            struct_default_init_with_query
        }
    }
}
