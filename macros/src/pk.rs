mod exists;
mod get;
mod take;
mod first;
mod last;
mod range;
mod pk_range;
mod store;
mod delete;
mod parent_pk;
mod limit;

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{Attribute, Data, DeriveInput, Field, Fields, Type};
use crate::field_parser::{Multiplicity, PkDef};
use crate::rest::FunctionDef;
use crate::macro_utils;
use crate::table::TableDef;

pub enum PointerType {
    Root,
    Child,
}

pub struct DbPkMacros {
    pub definition: PkDef,
    pub table_def: TableDef,
    pub query: Option<TokenStream>,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_defs: Vec<FunctionDef>,
}

impl DbPkMacros {
    pub fn new(entity_name: &Ident, entity_type: &Type, pk_def: &PkDef) -> Self {
        let pk_name: Ident = pk_def.field.name.clone();
        let pk_type = pk_def.field.tpe.clone();
        let table_def = TableDef::pk(entity_name, &pk_name, &pk_type);

        let mut function_defs: Vec<FunctionDef> = Vec::new();
        function_defs.push(get::fn_def(entity_name, entity_type, &pk_name, &pk_type, &table_def.name));
        function_defs.push(take::fn_def(entity_name, entity_type, &table_def.name));
        function_defs.push(first::fn_def(entity_name, entity_type, &table_def.name));
        function_defs.push(last::fn_def(entity_name, entity_type, &table_def.name));
        function_defs.push(limit::limit_fn_def(entity_name, entity_type));
        function_defs.push(exists::fn_def(entity_name, &pk_name, &pk_type, &table_def.name));

        match pk_def.fk {
            Some(Multiplicity::OneToMany) => {
                function_defs.push(parent_pk::fn_def(entity_name, &pk_name, &pk_type));
            }
            _ => {}
        };

        let entity_range_query = format_ident!("{}Range", entity_name.to_string());
        let mut range_query = None;

        if pk_def.range {
            function_defs.push(range::fn_def(entity_name, entity_type, &pk_name, &pk_type, &table_def.name, &entity_range_query));
            function_defs.push(pk_range::fn_def(entity_name, &pk_type, &table_def.name));
            range_query = Some(
                quote! {
                    #[derive(utoipa::IntoParams, serde::Serialize, serde::Deserialize, Default)]
                    pub struct #entity_range_query {
                        pub from: #pk_type,
                        pub until: #pk_type,
                    }
                    impl #entity_range_query {
                        pub fn sample() -> Vec<Self> {
                            vec![Self { from: #pk_type::default(), until: #pk_type::default() }]
                        }
                    }
                }
            );
        };

        DbPkMacros {
            definition: pk_def.clone(),
            table_def: table_def.clone(),
            query: range_query,
            store_statement: store::store_statement(&pk_name, &table_def.name),
            store_many_statement: store::store_many_statement(&pk_name, &table_def.name),
            delete_statement: delete::delete_statement(&table_def.name),
            delete_many_statement: delete::delete_many_statement(&table_def.name),
            function_defs,
        }
    }

    /// Determines whether a struct is a `Root` or `Child` based on `#[parent]` attributes.
    pub fn extract_pointer_type(input: &DeriveInput) -> Result<PointerType, syn::Error> {
        let data_struct = match &input.data {
            Data::Struct(data_struct) => data_struct,
            _ => return Err(syn::Error::new_spanned(input, "Pk can only be derived for structs")),
        };

        let fields: Vec<_> = match &data_struct.fields {
            Fields::Named(fields) => fields.named.iter().collect(),
            _ => return Err(syn::Error::new_spanned(input, "Pk can only be used with named fields")),
        };

        if fields.iter().any(|f| Self::has_parent_attribute(&f.attrs)) {
            Ok(PointerType::Child)
        } else {
            Ok(PointerType::Root)
        }
    }

    /// Extracts and validates the required fields (parent & index) for root or child structs.
    pub fn extract_fields(input: &DeriveInput, pointer_type: &PointerType) -> Result<(Option<Field>, Field), syn::Error> {
        let data_struct = match input.data.clone() {
            Data::Struct(data_struct) => data_struct,
            _ => return Err(syn::Error::new_spanned(input, "Pk can only be derived for structs")),
        };

        let fields: Vec<_> = match data_struct.fields {
            Fields::Named(fields) => fields.named.into_iter().collect(),
            _ => return Err(syn::Error::new_spanned(input, "Pk can only be used with named fields")),
        };

        match pointer_type {
            PointerType::Root => {
                if fields.len() != 1 {
                    return Err(syn::Error::new_spanned(input, "Root struct must have exactly one field"));
                }
                Ok((None, fields[0].clone())) // Root has only an index field
            }
            PointerType::Child => {
                if fields.len() != 2 {
                    return Err(syn::Error::new_spanned(input, "Child struct must have exactly two fields (parent and index)"));
                }

                let parent_field = match fields.iter().find(|f| Self::has_parent_attribute(&f.attrs)) {
                    Some(f) => f.clone(),
                    None => return Err(syn::Error::new_spanned(input, "Unable to find parent field")),
                };
                let index_field = match fields.iter().find(|f| !Self::has_parent_attribute(&f.attrs)) {
                    Some(f) => f.clone(),
                    None => return Err(syn::Error::new_spanned(input, "Unable to find index field")),
                };

                Ok((Some(parent_field), index_field))
            }
        }
    }

    /// Generates trait implementations for **Root Pointers** (IndexedPointer + RootPointer)
    /// and also derives Display, FromStr, Serialize, and Deserialize based on a dash-separated format.
    pub fn generate_root_impls(struct_name: &Ident, index_field: Field) -> TokenStream {
        let index_type = &index_field.ty;
        let index_name = &index_field.ident;

        let expanded = quote! {
            // Core traits
            impl IndexedPointer for #struct_name {
                type Index = #index_type;
                fn index(&self) -> Self::Index { self.#index_name }
                fn next(&self) -> Self { #struct_name { #index_name: self.#index_name + 1 } }
            }
            impl RootPointer for #struct_name {
                fn is_child(&self) -> bool { false }
            }

            // Serde: human-readable = dash string, binary = raw field
            impl serde::Serialize for #struct_name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: serde::Serializer {
                    if serializer.is_human_readable() {
                        serializer.serialize_str(&self.to_string())
                    } else {
                        #[derive(serde::Serialize)]
                        struct Helper {
                            #index_name: #index_type,
                        }
                        let helper = Helper { #index_name: self.#index_name };
                        helper.serialize(serializer)
                    }
                }
            }
            impl<'de> serde::Deserialize<'de> for #struct_name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: serde::Deserializer<'de> {
                    if deserializer.is_human_readable() {
                        let s = String::deserialize(deserializer)?;
                        // parse single int
                        let idx = s.parse::<#index_type>().map_err(serde::de::Error::custom)?;
                        Ok(#struct_name { #index_name: idx })
                    } else {
                        #[derive(serde::Deserialize)]
                        struct Helper {
                            #index_name: #index_type,
                        }
                        let helper = Helper::deserialize(deserializer)?;
                        Ok(#struct_name { #index_name: helper.#index_name })
                    }
                }
            }

            // HTTP path support
            impl std::fmt::Display for #struct_name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}", self.#index_name)
                }
            }
            impl std::str::FromStr for #struct_name {
                type Err = ParsePointerError;
                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    if s.contains('-') { return Err(ParsePointerError::Format); }
                    let idx = s.parse::<#index_type>()?;
                    Ok(#struct_name { #index_name: idx })
                }
            }

            impl redbit::utoipa::PartialSchema for #struct_name {
                fn schema() -> redbit::utoipa::openapi::RefOr<redbit::utoipa::openapi::schema::Schema> {
                    use redbit::utoipa::openapi::schema::*;
                    Schema::Object(
                        ObjectBuilder::new()
                            .schema_type(SchemaType::Type(Type::String))
                            .examples(vec!["0"])
                            .build()
                    ).into()
                }
            }

            impl redbit::utoipa::ToSchema for #struct_name {
                fn schemas(schemas: &mut Vec<(String, redbit::utoipa::openapi::RefOr<redbit::utoipa::openapi::schema::Schema>)>) {
                    schemas.push((stringify!(#struct_name).to_string(), <#struct_name as redbit::utoipa::PartialSchema>::schema()));
                }
            }
        };

        macro_utils::write_stream_and_return(expanded, struct_name)
    }

    /// Generate impls for Child Pointer types
    pub fn generate_child_impls(struct_name: &Ident, parent_field: Field, index_field: Field) -> TokenStream {
        let parent_name = &parent_field.ident;
        let parent_type = &parent_field.ty;
        let index_name = &index_field.ident;
        let index_type = &index_field.ty;

        let expanded = quote! {
            // Core traits
            impl IndexedPointer for #struct_name {
                type Index = #index_type;
                fn index(&self) -> Self::Index { self.#index_name }
                fn next(&self) -> Self { #struct_name { #parent_name: self.#parent_name.clone(), #index_name: self.#index_name + 1 } }
            }
            impl ChildPointer for #struct_name {
                type Parent = #parent_type;
                fn is_child(&self) -> bool { true }
                fn parent(&self) -> &Self::Parent { &self.#parent_name }
                fn from_parent(parent: Self::Parent) -> Self { #struct_name { #parent_name: parent, #index_name: <#index_type as Default>::default() } }
            }

            // Serde: human-readable = dash string, binary = raw fields
            impl serde::Serialize for #struct_name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: serde::Serializer {
                    if serializer.is_human_readable() {
                        serializer.serialize_str(&self.to_string())
                    } else {
                        #[derive(serde::Serialize)]
                        struct Helper {
                            #parent_name: #parent_type,
                            #index_name: #index_type,
                        }
                        let helper = Helper { #parent_name: self.#parent_name.clone(), #index_name: self.#index_name };
                        helper.serialize(serializer)
                    }
                }
            }
            impl<'de> serde::Deserialize<'de> for #struct_name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: serde::Deserializer<'de> {
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
                        #[derive(serde::Deserialize)]
                        struct Helper {
                            #parent_name: #parent_type,
                            #index_name: #index_type,
                        }
                        let helper = Helper::deserialize(deserializer)?;
                        Ok(#struct_name { #parent_name: helper.#parent_name, #index_name: helper.#index_name })
                    }
                }
            }

            // HTTP path support
            impl std::fmt::Display for #struct_name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}-{}", self.#parent_name, self.#index_name)
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

            impl redbit::utoipa::PartialSchema for #struct_name {
                fn schema() -> redbit::utoipa::openapi::RefOr<redbit::utoipa::openapi::schema::Schema> {
                    use redbit::utoipa::openapi::schema::*;
                    let example = format!("{}-{}", #parent_type::default(), "0");
                    Schema::Object(
                        ObjectBuilder::new()
                            .schema_type(SchemaType::Type(Type::String))
                            .examples(vec![example])
                            .build()
                    ).into()
                }
            }

            impl redbit::utoipa::ToSchema for #struct_name {
                fn schemas(schemas: &mut Vec<(String, redbit::utoipa::openapi::RefOr<redbit::utoipa::openapi::schema::Schema>)>) {
                    use redbit::utoipa::ToSchema;
                    schemas.push((stringify!(#struct_name).to_string(), <#struct_name as redbit::utoipa::PartialSchema>::schema()));
                    <#parent_type as ToSchema>::schemas(schemas);
                }
            }
        };

        macro_utils::write_stream_and_return(expanded, struct_name)
    }

    /// Checks if a field has the `#[parent]` attribute.
    fn has_parent_attribute(attrs: &[Attribute]) -> bool {
        attrs.iter().any(|attr| attr.path().is_ident("parent"))
    }
}
