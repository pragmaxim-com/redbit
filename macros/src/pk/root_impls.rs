use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Field;

/// Generates trait implementations for **Root Pointers** (IndexedPointer + RootPointer)
/// and also derives Display, FromStr, Serialize, and Deserialize based on a dash-separated format.
pub fn new(struct_name: &Ident, index_field: Field) -> TokenStream {
    let index_type = &index_field.ty;
    quote! {
        // Core traits
        impl IndexedPointer for #struct_name {
            type Index = #index_type;
            fn index(&self) -> Self::Index { self.0 }
            fn next(&self) -> Self { #struct_name(self.0 + 1) }
        }
        impl RootPointer for #struct_name {
            fn is_pointer(&self) -> bool { false }
        }

        impl UrlEncoded for #struct_name {
            fn encode(&self) -> String {
                format!("{}", self.0)
            }
        }
        
        impl std::str::FromStr for #struct_name {
            type Err = ParsePointerError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                if s.contains('-') { return Err(ParsePointerError::Format); }
                let idx = s.parse::<#index_type>()?;
                Ok(#struct_name(idx))
            }
        }

        impl PartialSchema for #struct_name {
            fn schema() -> openapi::RefOr<openapi::schema::Schema> {
                use openapi::schema::*;
                Schema::Object(
                    ObjectBuilder::new()
                        .schema_type(SchemaType::Type(Type::Integer))
                        .examples(vec![0])
                        .build()
                ).into()
            }
        }

        impl ToSchema for #struct_name {
            fn schemas(schemas: &mut Vec<(String, openapi::RefOr<openapi::schema::Schema>)>) {
                schemas.push((stringify!(#struct_name).to_string(), <#struct_name as PartialSchema>::schema()));
            }
        }
    }.into()

}
