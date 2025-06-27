use proc_macro2::TokenStream;
use quote::quote;
use crate::field_parser::ColumnDef;

pub struct TransientMacros {
    pub definition: ColumnDef,
    pub struct_default_init: TokenStream,
}

impl TransientMacros {
    pub fn new(defs: Vec<ColumnDef>) -> Vec<TransientMacros> {
        let mut transient_macros: Vec<TransientMacros> = Vec::new();
        for transient in defs {
            let field_name = &transient.field.name;
            let field_type = &transient.field.tpe;
            let struct_default_init = quote! {
                #field_name: (<#field_type>::default())
            };
            transient_macros.push(TransientMacros { definition: transient, struct_default_init})
        }
        transient_macros
    }
}
