use proc_macro2::{Literal, TokenStream as TokenStream2};
use quote::quote;
use syn::Type;
use crate::macro_utils::IntegerType;

pub(crate) fn emit_newtype_byte_array_impls(newtype_ty: &Type, len: usize) -> TokenStream2 {
    let mut tokens = TokenStream2::new();
    let n_lit = Literal::usize_unsuffixed(len);
    let inner_type_name = Literal::string(&format!("[u8;{}]", len));

    tokens.extend(quote! {
        impl redb::Value for #newtype_ty {
            type SelfType<'a> = #newtype_ty where Self: 'a;
            type AsBytes<'a> = &'a [u8; #n_lit] where Self: 'a;

            fn fixed_width() -> Option<usize> { Some(#n_lit) }

            fn from_bytes<'a>(data: &'a [u8]) -> #newtype_ty
            where Self: 'a {
                #newtype_ty(data.try_into().unwrap())
            }

            fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> &'a [u8; #n_lit]
            where Self: 'a, Self: 'b {
                &value.0
            }

            fn type_name() -> redb::TypeName { redb::TypeName::new(#inner_type_name) }
        }

        impl redb::Key for #newtype_ty {
            fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
                data1.cmp(data2)
            }
        }
    });

    tokens
}

pub(crate) fn emit_newtype_byte_vec_impls(newtype_ty: &Type) -> TokenStream2 {
    let mut tokens = TokenStream2::new();
    let inner_type_name = Literal::string("Vec<u8>");

    tokens.extend(quote! {
        impl redb::Value for #newtype_ty {
            type SelfType<'a> = #newtype_ty where Self: 'a;
            type AsBytes<'a> = &'a [u8] where Self: 'a;

            fn fixed_width() -> Option<usize> { None }

            fn from_bytes<'a>(data: &'a [u8]) -> #newtype_ty
            where Self: 'a {
                #newtype_ty(data.to_vec())
            }

            fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> &'a [u8]
            where Self: 'a, Self: 'b {
                value.0.as_ref()
            }

            fn type_name() -> redb::TypeName { redb::TypeName::new(#inner_type_name) }
        }

        impl redb::Key for #newtype_ty {
            fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
                data1.cmp(data2)
            }
        }
    });

    tokens
}

pub(crate) fn emit_newtype_integer_impls(newtype_ty: &Type, int_ty: &IntegerType) -> TokenStream2 {
    let mut tokens = TokenStream2::new();
    let int_str = int_ty.as_str(); // "u32", "i64", etc.
    let inner_name_lit = Literal::string(int_str);
    let int_ty_tokens: TokenStream2 = syn::parse_str(int_str).expect("valid integer type");

    tokens.extend(quote! {
        impl redb::Value for #newtype_ty {
            type SelfType<'a> = #newtype_ty where Self: 'a;
            type AsBytes<'a> = [u8; std::mem::size_of::<#int_ty_tokens>()] where Self: 'a;

            fn fixed_width() -> Option<usize> { Some(std::mem::size_of::<#int_ty_tokens>()) }

            fn from_bytes<'a>(data: &'a [u8]) -> #newtype_ty
            where Self: 'a {
                #newtype_ty(<#int_ty_tokens>::from_le_bytes(data.try_into().unwrap()))
            }

            fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> [u8; std::mem::size_of::<#int_ty_tokens>()]
            where Self: 'a, Self: 'b {
                value.0.to_le_bytes()
            }

            fn type_name() -> redb::TypeName { redb::TypeName::new(#inner_name_lit) }
        }

        impl redb::Key for #newtype_ty {
            fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
                let v1 = <#int_ty_tokens>::from_le_bytes(data1.try_into().unwrap());
                let v2 = <#int_ty_tokens>::from_le_bytes(data2.try_into().unwrap());
                v1.cmp(&v2)
            }
        }
    });

    tokens
}

pub(crate) fn emit_pointer_redb_impls(pointer_type: &Type) -> TokenStream2 {
    let mut tokens = TokenStream2::new();
    let inner_type_name = Literal::string(&quote!(#pointer_type).to_string());

    tokens.extend(quote! {
        impl redb::Value for #pointer_type {
            type SelfType<'a> = #pointer_type where Self: 'a;
            type AsBytes<'a> = std::borrow::Cow<'a, [u8]> where Self: 'a;

            fn fixed_width() -> Option<usize> {
                Some(<#pointer_type as BinaryCodec>::size())
            }

            fn from_bytes<'a>(data: &'a [u8]) -> #pointer_type
            where Self: 'a {
                <#pointer_type as BinaryCodec>::from_le_bytes(data)
            }

            fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
            where Self: 'a, Self: 'b {
                std::borrow::Cow::Owned(value.as_le_bytes())
            }

            fn type_name() -> redb::TypeName {
                redb::TypeName::new(#inner_type_name)
            }
        }

        impl redb::Key for #pointer_type {
            fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
                let a = <#pointer_type as BinaryCodec>::from_le_bytes(data1);
                let b = <#pointer_type as BinaryCodec>::from_le_bytes(data2);
                a.cmp(&b)
            }
        }
    });

    tokens
}

/// Last case: the *type itself* implements bincode::Encode + bincode::Decode<()>.
/// We encode/decode with bincode for persistence.
pub fn emit_newtype_bincode_impls(newtype_ty: &Type) -> TokenStream2 {
    let mut tokens = TokenStream2::new();

    tokens.extend(quote! {
        impl redb::Value for #newtype_ty {
            type SelfType<'a> = #newtype_ty where Self: 'a;
            // Bincode encoding allocates; expose owned bytes.
            type AsBytes<'a> = Vec<u8> where Self: 'a;

            fn fixed_width() -> Option<usize> { None }

            fn from_bytes<'a>(data: &'a [u8]) -> #newtype_ty
            where Self: 'a {
                // Requires: #newtype_ty: bincode::Decode<()>
                bincode::decode_from_slice::<#newtype_ty, _>(data, bincode::config::standard())
                    .unwrap()
                    .0
            }

            fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Vec<u8>
            where Self: 'a, Self: 'b {
                // Requires: #newtype_ty: bincode::Encode
                bincode::encode_to_vec(value, bincode::config::standard()).unwrap()
            }

            fn type_name() -> redb::TypeName {
                // Use the concrete type name; bincode layout depends on the type.
                redb::TypeName::new(std::any::type_name::<#newtype_ty>())
            }
        }

        impl redb::Key for #newtype_ty {
            fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
                let v1 = bincode::decode_from_slice::<#newtype_ty, _>(data1, bincode::config::standard())
                    .unwrap().0;
                let v2 = bincode::decode_from_slice::<#newtype_ty, _>(data2, bincode::config::standard())
                    .unwrap().0;
                v1.cmp(&v2)
            }
        }
    });

    tokens
}

pub(crate) fn emit_cachekey_byte_array_impls(newtype_ty: &Type, len: usize) -> TokenStream2 {
    let n = Literal::usize_unsuffixed(len);
    quote! {
        impl CacheKey for #newtype_ty {
            type CK = [u8; #n];

            #[inline]
            fn cache_key<'a>(v: &<#newtype_ty as redb::Value>::SelfType<'a>) -> Self::CK
            where
                #newtype_ty: 'a,
            {
                v.0
            }
        }
    }
}

pub(crate) fn emit_cachekey_byte_vec_impls(newtype_ty: &Type) -> TokenStream2 {
    quote! {
        impl CacheKey for #newtype_ty {
            type CK = Vec<u8>;

            #[inline]
            fn cache_key<'a>(v: &<#newtype_ty as redb::Value>::SelfType<'a>) -> Self::CK
            where
                #newtype_ty: 'a,
            {
                v.0.clone()
            }
        }
    }
}

pub(crate) fn emit_cachekey_integer_impls(newtype_ty: &Type, int_ty: &IntegerType) -> TokenStream2 {
    let int_str = int_ty.as_str(); // "u32", "i64", etc.
    let int_ty_tokens: TokenStream2 = syn::parse_str(int_str).expect("valid integer type");

    quote::quote! {
        // Cache key = the inner integer (Copy, Eq, Hash). Zero allocations.
        impl CacheKey for #newtype_ty {
            type CK = #int_ty_tokens;

            #[inline]
            fn cache_key<'a>(v: &<#newtype_ty as redb::Value>::SelfType<'a>) -> Self::CK
            where
                #newtype_ty: 'a,
            {
                v.0
            }
        }
    }
}

pub(crate) fn emit_cachekey_pointer_binarycodec_impls(pointer_ty: &Type) -> TokenStream2 {
    quote::quote! {
        // Cache key = serialized bytes via BinaryCodec.
        impl CacheKey for #pointer_ty {
            type CK = Vec<u8>;

            #[inline]
            fn cache_key<'a>(v: &<#pointer_ty as redb::Value>::SelfType<'a>) -> Self::CK
            where
                #pointer_ty: 'a,
            {
                // If your BinaryCodec uses `as_bytes`, switch to that.
                <#pointer_ty as BinaryCodec>::as_le_bytes(v)
            }
        }
    }
}

pub(crate) fn emit_cachekey_bincode_impls(newtype_ty: &Type) -> TokenStream2 {
    quote::quote! {
        // Cache key = bincode-encoded bytes. General and easy.
        impl CacheKey for #newtype_ty {
            type CK = Vec<u8>;

            #[inline]
            fn cache_key<'a>(v: &<#newtype_ty as redb::Value>::SelfType<'a>) -> Self::CK
            where
                #newtype_ty: 'a,
            {
                bincode::encode_to_vec(v, bincode::config::standard()).unwrap()
            }
        }
    }
}
