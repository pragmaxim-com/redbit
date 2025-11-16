use crate::macro_utils::IntegerType;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Type;

pub(crate) fn emit_newtype_byte_array_impls(newtype_ty: &Type, len: usize) -> TokenStream2 {
    quote! {
        impl_redb_newtype_array!(#newtype_ty, #len);
    }
}

pub(crate) fn emit_newtype_byte_vec_impls(newtype_ty: &Type) -> TokenStream2 {
    quote! {
        impl_redb_newtype_vec!(#newtype_ty);
    }
}

pub(crate) fn emit_newtype_integer_impls(newtype_ty: &Type, int_ty: &IntegerType) -> TokenStream2 {
    let int_str = int_ty.as_str(); // "u32", "i64", etc.
    let int_ty_tokens: TokenStream2 = syn::parse_str(int_str).expect("valid integer type");

    quote! {
        impl_redb_newtype_integer!(#newtype_ty, #int_ty_tokens);
    }
}

pub(crate) fn emit_pointer_redb_impls(pointer_type: &Type) -> TokenStream2 {
    quote! {
        impl_redb_newtype_binary!(#pointer_type);
    }
}

/// Last case: the *type itself* implements bincode::Encode + bincode::Decode<()>.
pub fn emit_newtype_bincode_impls(newtype_ty: &Type) -> TokenStream2 {
    quote! {
        impl_redb_newtype_bincode!(#newtype_ty);
    }
}

pub(crate) fn emit_cachekey_byte_array_impls(newtype_ty: &Type, len: usize) -> TokenStream2 {
    quote! {
        impl_cachekey_array!(#newtype_ty, #len);
    }
}

pub(crate) fn emit_cachekey_byte_vec_impls(newtype_ty: &Type) -> TokenStream2 {
    quote! {
        impl_cachekey_vec!(#newtype_ty);
    }
}

pub(crate) fn emit_cachekey_integer_impls(newtype_ty: &Type, int_ty: &IntegerType) -> TokenStream2 {
    let int_str = int_ty.as_str();
    let int_ty_tokens: TokenStream2 = syn::parse_str(int_str).expect("valid integer type");

    quote! {
        impl_cachekey_integer!(#newtype_ty, #int_ty_tokens);
    }
}

pub(crate) fn emit_cachekey_pointer_binarycodec_impls(pointer_ty: &Type) -> TokenStream2 {
    quote! {
        impl_cachekey_binary!(#pointer_ty);
    }
}

pub(crate) fn emit_cachekey_bincode_impls(newtype_ty: &Type) -> TokenStream2 {
    quote! {
        impl_cachekey_bincode!(#newtype_ty);
    }
}
