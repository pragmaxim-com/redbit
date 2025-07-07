use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Field;
use crate::macro_utils;

pub fn new(struct_name: &Ident, parent_field: Field, index_field: Field) -> TokenStream {
    let parent_name = &parent_field.ident;
    let parent_type = &parent_field.ty;
    let index_name = &index_field.ident;
    let index_type = &index_field.ty;
    let expanded = quote! {
        impl IndexedPointer for #struct_name {
            type Index = #index_type;
            fn index(&self) -> Self::Index { self.#index_name }
            fn next(&self) -> Self { #struct_name { #parent_name: self.#parent_name.clone(), #index_name: self.#index_name + 1 } }
        }
        impl ChildPointer for #struct_name {
            type Parent = #parent_type;
            fn is_pointer(&self) -> bool { true }
            fn parent(&self) -> &Self::Parent { &self.#parent_name }
            fn from_parent(parent: Self::Parent, index: #index_type) -> Self { #struct_name { #parent_name: parent, #index_name: index } }
        }

        impl Into<String> for #struct_name {
            fn into(self) -> String {
                self.encode()
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
            fn encode(&self) -> String {
                format!("{}-{}", self.#parent_name.encode(), self.#index_name)
            }
        }

        impl PartialSchema for #struct_name {
            fn schema() -> openapi::RefOr<openapi::schema::Schema> {
                use openapi::schema::*;
                Schema::Object(
                    ObjectBuilder::new()
                        .schema_type(SchemaType::Type(Type::String))
                        .examples(vec![Self::default().encode()])
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
    };

    macro_utils::write_stream_and_return(expanded, struct_name)
}
