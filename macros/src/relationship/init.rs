use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

pub fn o2o_relation_init(child_name: &Ident, child_type: &Type) -> TokenStream { 
    quote! {
        #child_name: #child_type::get(tx, pk)?.ok_or_else(|| AppError::NotFound(format!("Missing one-to-one child {:?}", pk)))?
    }
}

pub fn o2o_relation_default_init(child_name: &Ident, child_type: &Type) -> TokenStream { 
    quote! {
        #child_name: #child_type::sample_with(pk, sample_index)
    }
}

pub fn o2m_relation_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_name: {
            let (from, to) = pk.fk_range();
            #child_type::range(tx, &from, &to)?
        }
    }
}

pub fn o2m_relation_default_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_name:  {
            let (from, _) = pk.fk_range();
            let sample_0 = #child_type::sample_with(&from, sample_index);
            let sample_1 = #child_type::sample_with(&from.next(), sample_index);
            let sample_2 = #child_type::sample_with(&from.next().next(), sample_index);
            vec![sample_0, sample_1, sample_2]
        }
    }
}
