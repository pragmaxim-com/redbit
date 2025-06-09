use proc_macro2::TokenStream;
use quote::quote;
use syn::Type;

pub fn o2o_delete_def(child_type: &Type) -> TokenStream {
    quote! {
        #child_type::delete(&write_tx, pk)?;
    }
}

pub fn o2o_delete_many_def(child_type: &Type) -> TokenStream {
    quote! {
        #child_type::delete_many(&write_tx, pks)?;
    }
}

pub fn o2m_delete_def(child_type: &Type) -> TokenStream {
    quote! {
        let (from, to) = pk.fk_range();
        let child_pks = #child_type::pk_range(&write_tx, &from, &to)?;
        #child_type::delete_many(&write_tx, &child_pks)?;
    }
}

pub fn o2m_delete_many_def(child_type: &Type) -> TokenStream {
    quote! {
        let mut children = Vec::new();
        for pk in pks.iter() {
            let (from, to) = pk.fk_range();
            let child_pks = #child_type::pk_range(&write_tx, &from, &to)?;
            children.extend_from_slice(&child_pks);
        }
        #child_type::delete_many(&write_tx, &children)?;
    }
}
