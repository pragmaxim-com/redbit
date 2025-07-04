use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{ItemStruct, Type};
use crate::macro_utils;
use crate::macro_utils::InnerKind;

pub fn generate_column_impls(struct_ident: &Ident, index_new_type: &ItemStruct, inner_type: &Type) -> TokenStream {
    let kind = macro_utils::classify_inner_type(inner_type);

    let serialization_code = match kind {
        InnerKind::ByteArray(_) | InnerKind::VecU8 => quote! {
                if serializer.is_human_readable() {
                    serializer.serialize_str(&hex::encode(&self.0))
                } else {
                    self.0.serialize(serializer)
                }
            },
        _ => quote! {
                self.0.serialize(serializer)
            },
    };

    let deserialization_code = match kind {
        InnerKind::ByteArray(len) => quote! {
                if deserializer.is_human_readable() {
                    let s = <&str>::deserialize(deserializer)?;
                    let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
                    if bytes.len() != #len {
                        return Err(serde::de::Error::custom(format!("Invalid length: expected {} bytes, got {}", #len, bytes.len())));
                    }
                    let mut array = [0u8; #len];
                    array.copy_from_slice(&bytes);
                    Ok(#struct_ident(array))
                } else {
                    let inner = <#inner_type>::deserialize(deserializer)?;
                    Ok(#struct_ident(inner))
                }
            },
        InnerKind::VecU8 => quote! {
                if deserializer.is_human_readable() {
                    let s = <&str>::deserialize(deserializer)?;
                    let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
                    Ok(#struct_ident(bytes))
                } else {
                    let inner = <#inner_type>::deserialize(deserializer)?;
                    Ok(#struct_ident(inner))
                }
            },
        _ => quote! {
                let inner = <#inner_type>::deserialize(deserializer)?;
                Ok(#struct_ident(inner))
            },
    };

    let url_encoded_code = match kind {
        InnerKind::ByteArray(_) | InnerKind::VecU8 => quote! {
                format!("{}", hex::encode(&self.0))
            },
        _ => quote! {
                format!("{}", self.0)
            },
    };

    let default_code = match kind {
        InnerKind::String => quote! {
                Self("a".to_string())
            },
        InnerKind::VecU8 => quote! {
                Self(b"a".to_vec())
            },
        _ => quote! {
                Self(Default::default())
            },
    };

    let iterable_code = match kind {
        InnerKind::Integer => quote! {
                let next_val = self.0.wrapping_add(1);
                Self(next_val)
            },
        InnerKind::String => quote! {
                let mut bytes = self.0.clone().into_bytes();
                if let Some(last) = bytes.last_mut() {
                    *last = last.wrapping_add(1);
                } else {
                    bytes.push(1);
                }
                Self(String::from_utf8(bytes).expect("Invalid UTF-8"))
            },
        InnerKind::VecU8 => quote! {
                let mut vec = self.0.clone();
                if let Some(last) = vec.last_mut() {
                    *last = last.wrapping_add(1);
                } else {
                    vec.push(1);
                }
                Self(vec)
            },
        InnerKind::ByteArray(len) => quote! {
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
            },
        InnerKind::Other => quote! {
                compile_error!("IterableColumn not supported for this inner type");
            },
    };

    let (schema_type, schema_example) = match kind {
        InnerKind::ByteArray(_) | InnerKind::VecU8 => (
            quote! { SchemaType::Type(Type::String) },
            quote! { vec![Some(serde_json::json!(hex::encode(#struct_ident::default().0)))] },
        ),
        InnerKind::String => (
            quote! { SchemaType::Type(Type::String) },
            quote! { vec![Some(serde_json::json!(#struct_ident::default()))] },
        ),
        InnerKind::Integer => (
            quote! { SchemaType::Type(Type::Integer) },
            quote! { vec![Some(serde_json::json!(#struct_ident::default().0))] },
        ),
        _ => (
            quote! { SchemaType::Type(Type::String) },
            quote! { vec![Some(serde_json::json!(#struct_ident::default().0))] },
        ),
    };

    let expanded = quote! {
            #index_new_type
            impl Serialize for #struct_ident {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: Serializer {
                    #serialization_code
                }
            }

            impl<'de> Deserialize<'de> for #struct_ident {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: Deserializer<'de> {
                    #deserialization_code
                }
            }

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
                fn next(&self) -> Self {
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

            impl ToSchema for #struct_ident {
                fn schemas(schemas: &mut Vec<(String, openapi::RefOr<openapi::schema::Schema>)>) {
                    schemas.push((
                        stringify!(#struct_ident).to_string(),
                        <#struct_ident as PartialSchema>::schema()
                    ));
                }
            }
        };

    macro_utils::write_stream_and_return(expanded, struct_ident)
}
