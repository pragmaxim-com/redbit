use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse_str, Field, Type};
use crate::column::column_codec;

pub fn new(struct_name: &Ident, parent_field: Field, index_field: Field) -> TokenStream {
    let parent_name = &parent_field.ident;
    let parent_type = &parent_field.ty;
    let index_name = &index_field.ident;
    let index_type = &index_field.ty;
    let struct_type: Type = parse_str(&format!("{}", struct_name)).expect("Invalid Struct type");
    let custom_db_codec = column_codec::emit_pointer_redb_impls(&struct_type);
    quote! {
        #custom_db_codec
        impl IndexedPointer for #struct_name {
            type Index = #index_type;
            fn index(&self) -> Self::Index { self.#index_name }
            fn next_index(&self) -> Self { #struct_name { #parent_name: self.#parent_name.clone(), #index_name: self.#index_name + 1 } }
            fn nth_index(&self, n: usize) -> Self { #struct_name { #parent_name: self.#parent_name.clone(), #index_name: self.#index_name + n as #index_type} }
            fn rollback_or_init(&self, n: u32) -> Self {
                #struct_name { #parent_name: self.#parent_name.rollback_or_init(n), #index_name: 0 }
            }
        }
        impl ChildPointer for #struct_name {
            type Parent = #parent_type;
            fn is_pointer(&self) -> bool { true }
            fn parent(&self) -> Self::Parent { self.#parent_name }
            fn from_parent(parent: Self::Parent, index: #index_type) -> Self { #struct_name { #parent_name: parent, #index_name: index } }
        }

        impl BinaryCodec for #struct_name {
            fn from_bytes(bytes: &[u8]) -> Self {
                let parent_size = <#parent_type as BinaryCodec>::size();
                let index_size = std::mem::size_of::<#index_type>();
                assert_eq!(bytes.len(), parent_size + index_size, "invalid byte length for child pointer");
                let (parent_bytes, index_bytes) = bytes.split_at(parent_size);
                let parent = <#parent_type as BinaryCodec>::from_bytes(parent_bytes);
                let index_arr: [u8; std::mem::size_of::<#index_type>()] = index_bytes.try_into().unwrap();
                let index = <#index_type>::from_le_bytes(index_arr);
                #struct_name { #parent_name: parent, #index_name: index }
            }

            fn as_bytes(&self) -> Vec<u8> {
                let mut buf = self.#parent_name.as_bytes();
                buf.extend_from_slice(&self.#index_name.to_le_bytes());
                buf
            }
            fn size() -> usize {
                <#parent_type as BinaryCodec>::size() + std::mem::size_of::<#index_type>()
            }
        }

        impl Into<String> for #struct_name {
            fn into(self) -> String {
                self.url_encode()
            }
        }

        impl TryFrom<String> for #struct_name {
            type Error = ParsePointerError;
            fn try_from(s: String) -> Result<Self, Self::Error> {
                let mut parts = s.rsplitn(2, '-');
                let idx_str    = parts.next().ok_or(ParsePointerError::Format)?;
                let parent_str = parts.next().ok_or(ParsePointerError::Format)?;
                let parent     = parent_str.parse::<#parent_type>()?;
                let idx        = idx_str.parse::<#index_type>()?;
                Ok(#struct_name { #parent_name: parent, #index_name: idx })
            }
        }

        impl core::str::FromStr for #struct_name {
            type Err = ParsePointerError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::try_from(s.to_string())
            }
        }

        impl UrlEncoded for #struct_name {
            fn url_encode(&self) -> String {
                format!("{}-{}", self.#parent_name.url_encode(), self.#index_name)
            }
        }

        impl PartialSchema for #struct_name {
            fn schema() -> openapi::RefOr<openapi::schema::Schema> {
                use openapi::schema::*;
                use openapi::extensions::ExtensionsBuilder;
                Schema::Object(
                    ObjectBuilder::new()
                        .schema_type(SchemaType::Type(Type::String))
                        .examples(vec![Self::default().url_encode()])
                        .extensions(Some(ExtensionsBuilder::new().add("key", "fk").build()))
                        .build()
                ).into()
            }
        }

        impl ToSchema for #struct_name {
            fn schemas(schemas: &mut Vec<(String, openapi::RefOr<openapi::schema::Schema>)>) {
                schemas.push((stringify!(#struct_name).to_string(), <#struct_name as PartialSchema>::schema()));
                <#parent_type as ToSchema>::schemas(schemas);
            }
        }
    }
}
