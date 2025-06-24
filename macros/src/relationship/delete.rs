use proc_macro2::TokenStream;
use quote::quote;
use syn::Type;

pub fn one2one_delete_def(child_type: &Type) -> TokenStream {
    quote! {
        #child_type::delete(&tx, pk)?;
    }
}

pub fn one2one_delete_many_def(child_type: &Type) -> TokenStream {
    quote! {
        #child_type::delete_many(&tx, pks)?;
    }
}


pub fn one2option_delete_def(child_type: &Type) -> TokenStream {
    quote! {
        #child_type::delete(&tx, pk)?;
    }
}

pub fn one2option_delete_many_def(child_type: &Type) -> TokenStream {
    quote! {
        #child_type::delete_many(&tx, pks)?;
    }
}


pub fn one2many_delete_def(child_type: &Type) -> TokenStream {
    quote! {
        let (from, to) = pk.fk_range();
        let child_pks = #child_type::pk_range(&tx, &from, &to)?;
        #child_type::delete_many(&tx, &child_pks)?;
    }
}

pub fn one2many_delete_many_def(child_type: &Type) -> TokenStream {
    quote! {
        let mut children = Vec::new();
        for pk in pks.iter() {
            let (from, to) = pk.fk_range();
            let child_pks = #child_type::pk_range(&tx, &from, &to)?;
            children.extend_from_slice(&child_pks);
        }
        #child_type::delete_many(&tx, &children)?;
    }
}
