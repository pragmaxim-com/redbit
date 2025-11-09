use crate::column::column_codec;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse_str, Field, Type};

/// Generates trait implementations for **Root Pointers** (IndexedPointer + RootPointer)
/// and also derives Display, FromStr, Serialize, and Deserialize based on a dash-separated format.
pub fn new(struct_name: &Ident, index_field: Field) -> TokenStream {
    let index_type = &index_field.ty;
    let struct_type: Type = parse_str(&format!("{}", struct_name)).expect("Invalid Struct type");
    let custom_db_codec = column_codec::emit_pointer_redb_impls(&struct_type);
    let cache_key_codec = column_codec::emit_cachekey_pointer_binarycodec_impls(&struct_type);
    quote! {
        #custom_db_codec
        #cache_key_codec
        impl ColInnerType for #struct_name {
            type Repr = #index_type;
        }
        impl IndexedPointer for #struct_name {
            type Index = #index_type;
            fn index(&self) -> Self::Index { self.0 }
            fn next_index(&self) -> Self { #struct_name(self.0 + 1) }
            fn nth_index(&self, n: usize) -> Self { #struct_name(self.0 + n as #index_type) }
            fn rollback_or_init(&self, n: u32) -> Self {
                let prev_index = self.0.checked_sub(n).unwrap_or(0);
                #struct_name(prev_index)
            }
        }
        impl RootPointer for #struct_name {
            fn total_index(&self) -> u128 {
                self.index().into()
            }
            fn is_pointer(&self) -> bool { false }
            fn from_many(pks: &[#index_type]) -> Vec<Self> {
                pks.iter().map(|idx| #struct_name(*idx)).collect()
            }
            fn depth(&self) -> usize {
                0
            }
        }

        impl BinaryCodec for #struct_name {
            fn from_le_bytes(bytes: &[u8]) -> Self {
                let arr: [u8; std::mem::size_of::<#index_type>()] = bytes.try_into().expect("invalid byte length for index");
                Self(<#index_type>::from_le_bytes(arr))
            }

            fn as_le_bytes(&self) -> Vec<u8> {
                self.0.to_le_bytes().to_vec()
            }

            fn size() -> usize {
                std::mem::size_of::<#index_type>()
            }
        }

        impl_copy_owned_value_identity!(#struct_name);

        impl UrlEncoded for #struct_name {
            fn url_encode(&self) -> String {
                format!("{}", self.0)
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
                if s.contains('-') { return Err(ParsePointerError::Format); }
                let idx = s.parse::<#index_type>()?;
                Ok(#struct_name(idx))
            }
        }

        impl std::str::FromStr for #struct_name {
            type Err = ParsePointerError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::try_from(s.to_string())
            }
        }

        impl PartialSchema for #struct_name {
            fn schema() -> openapi::RefOr<Schema> {
                rest::schema(SchemaType::Type(Type::Integer), vec![0], Some(ExtensionsBuilder::new().add("key", "pk").build()))
            }
        }

        impl ToSchema for #struct_name {
            fn schemas(schemas: &mut Vec<(String, openapi::RefOr<openapi::schema::Schema>)>) {
                schemas.push((stringify!(#struct_name).to_string(), <#struct_name as PartialSchema>::schema()));
            }
        }
    }

}
