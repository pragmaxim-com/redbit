use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Field;
use crate::macro_utils;

/// Generates trait implementations for **Root Pointers** (IndexedPointer + RootPointer)
/// and also derives Display, FromStr, Serialize, and Deserialize based on a dash-separated format.
pub fn new(struct_name: &Ident, index_field: Field) -> TokenStream {
    let index_type = &index_field.ty;

    let expanded = quote! {
            // Core traits
            impl IndexedPointer for #struct_name {
                type Index = #index_type;
                fn index(&self) -> Self::Index { self.0 }
                fn next(&self) -> Self { #struct_name(self.0 + 1) }
            }
            impl RootPointer for #struct_name {
                fn is_pointer(&self) -> bool { false }
            }

            // Serde: human-readable = dash string, binary = raw field
            impl Serialize for #struct_name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: Serializer {
                    if serializer.is_human_readable() {
                        self.0.serialize(serializer)
                    } else {
                        #[derive(Serialize)]
                        struct Helper(#index_type);
                        let helper = Helper(self.0);
                        helper.serialize(serializer)
                    }
                }
            }
            impl<'de> Deserialize<'de> for #struct_name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: Deserializer<'de> {
                    if deserializer.is_human_readable() {
                        let idx = #index_type::deserialize(deserializer)?;
                        Ok(#struct_name(idx))
                    } else {
                        #[derive(Deserialize)]
                        struct Helper(#index_type);
                        let helper = Helper::deserialize(deserializer)?;
                        Ok(#struct_name(helper.0))
                    }
                }
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
        };

    macro_utils::write_stream_and_return(expanded, struct_name)
}
