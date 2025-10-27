use crate::column::column_codec::*;
use crate::macro_utils;
use crate::macro_utils::InnerKind;
use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{parse_str, Attribute, FieldsNamed, Path, Type};

pub fn generate_column_impls(
    struct_ident: &Ident,
    new_type: &Type,
    inner_type: &Type,
    binary_encoding_opt: Option<String>,
) -> (TokenStream, Option<Attribute>, Punctuated<Path, Comma>) {
    let kind = macro_utils::classify_inner_type(inner_type);

    let binary_encoding = binary_encoding_opt.unwrap_or_else(|| "hex".to_string());
    let mut schema_example = quote! { vec![Some(json!(#struct_ident::default().url_encode()))] };
    let mut struct_attr: Option<Attribute> = None;
    let mut extra_derive_impls: Punctuated<Path, Comma> = Punctuated::new();
    let mut schema_type = quote! { SchemaType::Type(Type::String) };
    let mut default_code = quote! { Self(Default::default()) };
    let mut url_encoded_code = quote! { format!("{}", self.0) };
    let mut iterable_code = quote! { compile_error!("Sampleable::next is not supported for this type.") };
    let mut custom_db_codec = quote! {};
    let mut cache_key_codec = quote! {};

    match kind {
        InnerKind::ByteArray(len) => {
            let(encoding, example) = match binary_encoding.as_ref() {
                "hex" => ("serde_with::hex::Hex", quote! { Self([0u8; #len]) }),
                "base64" => ("serde_with::base64::Base64", quote! { Self([0u8; #len]) }),
                "utf-8" => ("redbit::utf8_serde_enc::Utf8", quote! { Self([0u8; #len]) }),
                custom=> {
                    let (encoding, example) =
                        custom
                            .split_once(' ')
                            .expect("Expected encoding 'hex', 'base64' or 'utf-8' are supported");
                    (encoding, quote! { Self(#example.as_bytes().to_vec()) })
                },
            };
            let binary_encoding_literal = Literal::string(encoding);
            default_code = example;
            struct_attr = Some(syn::parse_quote! { #[serde_as(as = #binary_encoding_literal)] });
            url_encoded_code = quote! { serde_json::to_string(&self).unwrap().trim_matches('"').to_string() };
            extra_derive_impls.push(syn::parse_quote!(Copy));
            custom_db_codec = emit_newtype_byte_array_impls(new_type, len);
            cache_key_codec = emit_cachekey_byte_array_impls(new_type, len);
            iterable_code = quote! {
                let mut arr = self.0;
                for i in (0..#len).rev() {
                    if arr[i] != 0xFF {
                        arr[i] = arr[i].wrapping_add(1);
                        break;
                    } else {
                        arr[i] = 0;
                    }
                }
                Self(arr)
            };
        }
        InnerKind::VecU8 => {
            let encoding = match binary_encoding.as_ref() {
                "hex"               => "redbit::hex_serde_enc::Hex",
                "base64"            => "redbit::base64_serde_enc::Base64",
                "utf-8"             => "redbit::utf8_serde_enc::Utf8",
                custom=> custom,

            };
            let ty: syn::Path = parse_str(encoding).expect("Invalid Encoding type");
            let binary_encoding_literal = Literal::string(encoding);
            default_code = quote! { Self(<#ty as ByteVecColumnSerde>::decoded_example()) };
            struct_attr = Some(syn::parse_quote! { #[serde_as(as = #binary_encoding_literal)] });
            url_encoded_code = quote! { serde_json::to_string(&self).unwrap().trim_matches('"').to_string() };
            custom_db_codec = emit_newtype_byte_vec_impls(new_type);
            cache_key_codec = emit_cachekey_byte_vec_impls(new_type);
            iterable_code = quote! {
                Self(<#ty as ByteVecColumnSerde>::next_value(&self.0))
            };
        }
        InnerKind::Integer(int_type) => {
            schema_type = quote! { SchemaType::Type(Type::Integer) };
            iterable_code = quote! { Self(self.0.wrapping_add(1)) };
            schema_example = quote! { vec![Some(0)] };
            extra_derive_impls.push(syn::parse_quote![Copy]);
            custom_db_codec = emit_newtype_integer_impls(new_type, &int_type);
            cache_key_codec = emit_cachekey_integer_impls(new_type, &int_type);
        }
        InnerKind::String => {
            default_code = quote! { Self("a".to_string()) };
            iterable_code = quote! {
                let mut bytes = self.0.clone().into_bytes();
                if let Some(last) = bytes.last_mut() {
                    *last = last.wrapping_add(1);
                } else {
                    bytes.push(1);
                }
                Self(String::from_utf8(bytes).expect("Invalid UTF-8"))
            };
            custom_db_codec = emit_newtype_bincode_impls(new_type);
            cache_key_codec = emit_cachekey_bincode_impls(new_type);
        }
        InnerKind::Bool => {
            schema_type = quote! { SchemaType::Type(Type::Boolean) };
            default_code = quote! { Self(false) };
            url_encoded_code = quote! { self.0.to_string() };
            iterable_code = quote! { Self(!self.0) };
            extra_derive_impls.push(syn::parse_quote!(Copy));
            custom_db_codec = emit_newtype_bincode_impls(new_type);
            cache_key_codec = emit_cachekey_bincode_impls(new_type);
        }
        InnerKind::Uuid => {
            default_code = quote! { Self(uuid::Uuid::nil()) };
            url_encoded_code = quote! { self.0.to_string() };
            iterable_code = quote! {
                let mut bytes = *self.0.as_bytes();
                for i in (0..bytes.len()).rev() {
                    if bytes[i] != 0xFF {
                        bytes[i] = bytes[i].wrapping_add(1);
                        break;
                    } else {
                        bytes[i] = 0;
                    }
                }
                Self(uuid::Uuid::from_bytes(bytes))
            };
            custom_db_codec = emit_newtype_bincode_impls(new_type);
            cache_key_codec = emit_cachekey_bincode_impls(new_type);
        }
/*        InnerKind::UtcDateTime => {
            struct_attr = Some(syn::parse_quote! { #[serde_as(as = "serde_with::TimestampMilliSeconds<i64>")] });
            default_code = quote! { Self(chrono::DurationRound::duration_trunc(chrono::Utc::now(), chrono::TimeDelta::hours(1)).unwrap().to_utc()) };
            schema_type = quote! { SchemaType::Type(Type::Integer) };
            url_encoded_code = quote! { format!("{}", self.0.timestamp_millis()) };
            schema_example = quote! { vec![Some(0)] };
            iterable_code = quote! { Self(self.0 + chrono::Duration::milliseconds(1)) };
            custom_db_codec = emit_newtype_bincode_impls(new_type);
        }
*/        InnerKind::Time => {
            struct_attr = Some(syn::parse_quote! { #[serde_as(as = "serde_with::DurationMilliSeconds")] });
            default_code = quote! { Self(std::time::Duration::from_secs(0)) };
            schema_type = quote! { SchemaType::Type(Type::Integer) };
            url_encoded_code = quote! { format!("{}", self.0.as_millis()) };
            schema_example = quote! { vec![Some(0)] };
            iterable_code = quote! { Self(self.0 + std::time::Duration::from_millis(1)) };
            custom_db_codec = emit_newtype_bincode_impls(new_type);
            cache_key_codec = emit_cachekey_bincode_impls(new_type);
        }
/*        InnerKind::EnumReprU8 => {
            struct_attr = Some(syn::parse_quote! { #[serde_as(as = "serde_with::DisplayFromStr")] }, );
            url_encoded_code = quote! { format!("{}", self.0) };
            iterable_code = quote! {
                // Increment the underlying repr u8, wrapping around at 255
                let val = (self.0 as u8).wrapping_add(1);
                Self(unsafe { std::mem::transmute::<u8, Self>(val) })
            };
        }
*/        InnerKind::Other => {
            // leave defaults (compile error for next)
        }
    }

    let impls = quote! {
        #custom_db_codec
        #cache_key_codec

        impl ColInnerType for #struct_ident {
            type Repr = #inner_type;
        }

        impl UrlEncoded for #struct_ident {
            fn url_encode(&self) -> String {
                #url_encoded_code
            }
        }

        impl Default for #struct_ident {
            fn default() -> Self {
                #default_code
            }
        }

        impl Sampleable for #struct_ident {
            fn next_value(&self) -> Self {
                #iterable_code
            }
        }

        impl PartialSchema for #struct_ident {
            fn schema() -> openapi::RefOr<openapi::schema::Schema> {
                use openapi::schema::*;
                Schema::Object(
                    ObjectBuilder::new()
                        .schema_type(#schema_type)
                        .examples(#schema_example)
                        .build()
                ).into()
            }
        }

        impl utoipa::ToSchema for #struct_ident {
            fn schemas(schemas: &mut Vec<(String, openapi::RefOr<openapi::schema::Schema>)>) {
                schemas.push((
                    stringify!(#struct_ident).to_string(),
                    <#struct_ident as PartialSchema>::schema()
                ));
            }
        }
    };

    (impls, struct_attr, extra_derive_impls)
}

fn make_zero_or_default_field_expr(field_ty: &Type) -> TokenStream {
    if macro_utils::classify_integer_type(field_ty).is_some() {
        quote! { 0 as #field_ty }
    } else {
        quote! { Default::default() }
    }
}

fn gen_field_inits_zero(fields: &FieldsNamed) -> Vec<TokenStream> {
    fields.named.iter().map(|f| {
        let name = f.ident.as_ref().expect("named field");
        let ty   = &f.ty;
        let expr = make_zero_or_default_field_expr(ty);
        quote! { #name: #expr, }
    }).collect()
}

fn make_next_field_expr(field_ident: &Ident, field_ty: &Type) -> TokenStream {
    if macro_utils::classify_integer_type(field_ty).is_some() {
        quote! { self.#field_ident.wrapping_add(1) }
    } else {
        quote! { <#field_ty as Sampleable>::next_value(&self.#field_ident) }
    }
}

pub fn gen_default_impl(struct_ident: &Ident, struct_type: &Type, fields: &FieldsNamed) -> TokenStream {
    let default_inits = gen_field_inits_zero(fields);

    quote! {
        impl ::core::default::Default for #struct_type {
            fn default() -> Self {
                #struct_ident { #(#default_inits)* }
            }
        }
    }
}

pub fn gen_sampleable_impl(struct_ident: &Ident, struct_type: &Type, fields: &FieldsNamed) -> TokenStream {
    // next_value: advance all fields (ints += 1; non-ints delegate to Sampleable::next_value)
    let next_full_inits: Vec<TokenStream> = fields.named.iter().map(|f| {
        let name = f.ident.as_ref().expect("named field");
        let ty   = &f.ty;
        let expr = make_next_field_expr(name, ty);
        quote! { #name: #expr, }
    }).collect();

    // step_index_only: only integer fields increment; non-integer fields stay constant (clone)
    let next_index_only_inits: Vec<TokenStream> = fields.named.iter().map(|f| {
        let name = f.ident.as_ref().expect("named field");
        let ty   = &f.ty;
        if macro_utils::classify_integer_type(ty).is_some() {
            quote! { #name: self.#name.wrapping_add(1), }
        } else {
            quote! { #name: self.#name.clone(), }
        }
    }).collect();

    // seed_nth_with_index_zero: start from Default().nth_value(from), then force `index` = 0
    let index_zero_stmt: Vec<TokenStream> = fields.named.iter().filter_map(|f| {
        let name = f.ident.as_ref().expect("named field");
        let ty   = &f.ty;
        if name == "index" && macro_utils::classify_integer_type(ty).is_some() {
            Some(quote! { seed.#name = 0 as #ty; })
        } else {
            None
        }
    }).collect();

    quote! {
        impl Sampleable for #struct_type {
            #[inline]
            fn next_value(&self) -> Self {
                #struct_ident { #(#next_full_inits)* }
            }
            #[inline]
            fn sample_many_from(n: usize, from: usize) -> Vec<Self> {
                Self::sample_many_from_seed_index_only(n, &Self::seed_nth_with_index_zero(from))
            }
        }

        impl #struct_ident {
            #[inline]
            fn step_index_only(&self) -> Self {
                #struct_ident { #(#next_index_only_inits)* }
            }

            #[inline]
            pub fn seed_nth_with_index_zero(from: usize) -> Self
            where #struct_type: Default + Sampleable + Clone
            {
                let mut seed = <#struct_type as Default>::default().nth_value(from);
                #(#index_zero_stmt)*
                seed
            }

            #[inline]
            pub fn sample_many_from_seed_index_only(n: usize, seed: &Self) -> Vec<Self> {
                let mut out = Vec::with_capacity(n);
                let mut v = seed.clone();
                for _ in 0..n {
                    out.push(v.clone());
                    v = v.step_index_only();
                }
                out
            }
        }
    }
}
