use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Field;
use crate::macro_utils;

pub fn new(struct_name: &Ident, parent_field: Field, index_field: Field) -> TokenStream {
    let parent_name = &parent_field.ident;
    let parent_type = &parent_field.ty;
    let index_name = &index_field.ident;
    let index_type = &index_field.ty;
    let helper_name = Ident::new(&format!("{}Helper", struct_name), struct_name.span());
    let expanded = quote! {
            #[derive(Serialize, Deserialize)]
            struct #helper_name {
                #parent_name: #parent_type,
                #index_name: #index_type,
            }

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

            // Serde: human-readable = dash string, binary = raw fields
            impl Serialize for #struct_name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: Serializer {
                    if serializer.is_human_readable() {
                        serializer.serialize_str(UrlEncoded::encode(self).as_str())
                    } else {
                        let helper = #helper_name { #parent_name: self.#parent_name.clone(), #index_name: self.#index_name };
                        helper.serialize(serializer)
                    }
                }
            }
            impl<'de> Deserialize<'de> for #struct_name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: Deserializer<'de> {
                    if deserializer.is_human_readable() {
                        let s = String::deserialize(deserializer)?;
                        // split on last dash
                        let mut parts = s.rsplitn(2, '-');
                        let idx_str = parts.next().ok_or_else(|| serde::de::Error::custom(ParsePointerError::Format))?;
                        let parent_str = parts.next().ok_or_else(|| serde::de::Error::custom(ParsePointerError::Format))?;
                        let parent = parent_str.parse::<#parent_type>().map_err(serde::de::Error::custom)?;
                        let idx = idx_str.parse::<#index_type>().map_err(serde::de::Error::custom)?;
                        Ok(#struct_name { #parent_name: parent, #index_name: idx })
                    } else {
                        let helper = #helper_name::deserialize(deserializer)?;
                        Ok(#struct_name { #parent_name: helper.#parent_name, #index_name: helper.#index_name })
                    }
                }
            }

            impl UrlEncoded for #struct_name {
                fn encode(&self) -> String {
                    format!("{}-{}", self.#parent_name.encode(), self.#index_name)
                }
            }

            impl std::str::FromStr for #struct_name {
                type Err = ParsePointerError;
                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    let mut parts = s.rsplitn(2, '-');
                    let idx_str = parts.next().ok_or(ParsePointerError::Format)?;
                    let parent_str = parts.next().ok_or(ParsePointerError::Format)?;
                    let parent = parent_str.parse::<#parent_type>()?;
                    let idx = idx_str.parse::<#index_type>()?;
                    Ok(#struct_name { #parent_name: parent, #index_name: idx })
                }
            }

            impl PartialSchema for #struct_name {
                fn schema() -> openapi::RefOr<openapi::schema::Schema> {
                    use openapi::schema::*;
                    let example = format!("{}-{}", #parent_type::default().encode(), "0");
                    Schema::Object(
                        ObjectBuilder::new()
                            .schema_type(SchemaType::Type(Type::String))
                            .examples(vec![example])
                            .build()
                    ).into()
                }
            }

            impl ToSchema for #struct_name {
                fn schemas(schemas: &mut Vec<(String, openapi::RefOr<openapi::schema::Schema>)>) {
                    use ToSchema;
                    schemas.push((stringify!(#struct_name).to_string(), <#struct_name as PartialSchema>::schema()));
                    <#parent_type as ToSchema>::schemas(schemas);
                }
            }
        };

    macro_utils::write_stream_and_return(expanded, struct_name)
}
