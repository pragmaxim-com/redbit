use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;

pub fn one2one_relation_expr(child_type: &Type) -> TokenStream {
    quote! {
        #child_type::get(tx, pk)?.ok_or_else(|| AppError::NotFound(format!("Missing one-to-one child {:?}", pk)))?
    }
}

pub fn one2one_relation_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    let init_expr = one2one_relation_expr(child_type);
    quote! {
        #child_name: #init_expr
    }
}

pub fn one2one_relation_init_with_query(
    child_name: &Ident,
    child_type: &Type,
) -> TokenStream {
    let init_expr = one2one_relation_expr(child_type);
    quote! {
        let #child_name = #init_expr;
        // todo!!!
    }
}

pub fn one2one_relation_default_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_name: #child_type::sample_with(pk, sample_index)
    }
}

pub fn one2opt_relation_expr(child_type: &Type) -> TokenStream {
    quote! {
        #child_type::get(tx, pk)?
    }
}

pub fn one2opt_relation_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    let init_expr = one2opt_relation_expr(child_type);
    quote! {
        #child_name: #init_expr
    }
}

pub fn one2opt_relation_init_with_query(
    child_name: &Ident,
    child_type: &Type,
) -> TokenStream {
    let init_expr = one2opt_relation_expr(child_type);
    quote! {
        let #child_name = #init_expr;
        // todo!!!
    }
}

pub fn one2opt_relation_default_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    quote! {
        #child_name: Some(#child_type::sample_with(pk, sample_index))
    }
}

pub fn one2many_relation_expr(child_type: &Type) -> TokenStream {
    quote! {
        {
            let (from, to) = pk.fk_range();
            #child_type::range(tx, &from, &to)?
        }
    }
}

pub fn one2many_relation_init_with_query(
    child_name: &Ident,
    child_type: &Type,
) -> TokenStream {
    let init_expr = one2many_relation_expr(child_type);
    quote! {
        let #child_name = #init_expr;
        // todo!!!
    }
}

pub fn one2many_relation_init(child_name: &Ident, child_type: &Type) -> TokenStream {
    let init_expr = one2many_relation_expr(child_type);
    quote! {
        #child_name: #init_expr
    }
}

pub fn one2many_relation_default_init(child_name: &Ident, child_type: &Type) -> TokenStream {
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
