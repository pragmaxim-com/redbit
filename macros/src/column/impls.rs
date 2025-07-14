use proc_macro2::{Literal, Ident, TokenStream};
use quote::quote;
use syn::{Attribute, Type};

use crate::macro_utils::InnerKind;

pub fn generate_column_impls(
    struct_ident: &Ident,
    inner_type: &Type,
    binary_encoding_opt: Option<Literal>,
) -> (TokenStream, Option<Attribute>) {
    let hex_encoding = Literal::string("hex");
    let base64_encoding = Literal::string("base64");
    let kind = crate::macro_utils::classify_inner_type(inner_type);
    let encoding_raw = binary_encoding_opt.unwrap_or_else(|| hex_encoding.clone());
    let binary_encoding_literal = match encoding_raw {
        l if l.to_string() == hex_encoding.to_string() =>
            Literal::string(&"serde_with::hex::Hex"),
        l if l.to_string() == base64_encoding.to_string() =>
            Literal::string(&"serde_with::base64::Base64"),
        _ => panic!("Unknown encoding '{}'. Expected 'hex' or 'base64'.", encoding_raw),
    };

    let schema_example = quote! { vec![Some(serde_json::json!(#struct_ident::default().encode()))] };
    let mut struct_attr: Option<Attribute> = None;
    let mut schema_type = quote! { SchemaType::Type(Type::String) };
    let mut default_code = quote! { Self(Default::default()) };
    let mut url_encoded_code = quote! { format!("{}", self.0) };
    let mut iterable_code = quote! { compile_error!("IterableColumn::next is not supported for this type.") };

    match kind {
        InnerKind::ByteArray(len) => {
            struct_attr = Some(syn::parse_quote! { #[serde_as(as = #binary_encoding_literal)] });
            default_code = quote! { Self([0u8; #len]) };
            url_encoded_code = quote! { serde_json::to_string(&self).unwrap().trim_matches('"').to_string() };
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
            struct_attr = Some(syn::parse_quote! { #[serde_as(as = #binary_encoding_literal)] });
            default_code = quote! { Self(b"a".to_vec()) };
            url_encoded_code = quote! { serde_json::to_string(&self).unwrap().trim_matches('"').to_string() };
            iterable_code = quote! {
                let mut vec = self.0.clone();
                if let Some(last) = vec.last_mut() {
                    *last = last.wrapping_add(1);
                } else {
                    vec.push(1);
                }
                Self(vec)
            };
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
        }
        InnerKind::Integer => {
            schema_type = quote! { SchemaType::Type(Type::Integer) };
            iterable_code = quote! { Self(self.0.wrapping_add(1)) };
        }
        InnerKind::Bool => {
            schema_type = quote! { SchemaType::Type(Type::Boolean) };
            default_code = quote! { Self(false) };
            url_encoded_code = quote! { self.0.to_string() };
            iterable_code = quote! { Self(!self.0) };
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
        }
        InnerKind::UtcDateTime => {
            struct_attr = Some(syn::parse_quote! { #[serde_as(as = "serde_with::TimestampMilliSeconds<i64>")] });
            default_code = quote! { Self(chrono::DurationRound::duration_trunc(chrono::Utc::now(), chrono::TimeDelta::hours(1)).unwrap().to_utc()) };
            schema_type = quote! { SchemaType::Type(Type::Integer) };
            url_encoded_code = quote! { format!("{}", self.0.timestamp_millis()) };
            iterable_code = quote! { Self(self.0 + chrono::Duration::milliseconds(1)) };
        }
        InnerKind::Time => {
            struct_attr = Some(syn::parse_quote! { #[serde_as(as = "serde_with::DurationMilliSeconds")] });
            default_code = quote! { Self(std::time::Duration::from_secs(0)) };
            schema_type = quote! { SchemaType::Type(Type::Integer) };
            url_encoded_code = quote! { format!("{}", self.0.as_millis()) };
            iterable_code = quote! { Self(self.0 + std::time::Duration::from_millis(1)) };
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
        impl UrlEncoded for #struct_ident {
            fn encode(&self) -> String {
                #url_encoded_code
            }
        }

        impl Default for #struct_ident {
            fn default() -> Self {
                #default_code
            }
        }

        impl IterableColumn for #struct_ident {
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

    (impls, struct_attr)
}
