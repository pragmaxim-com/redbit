mod delete;
mod stream_by;
mod init;
mod stream_range_by;
mod store;
mod stream_keys_by;
mod query;
mod range_by;
mod get_by;
mod get_keys_by;

use crate::field_parser::{FieldDef, IndexingType};
use crate::macro_utils;
use crate::rest::*;
use crate::table::TableDef;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{ItemStruct, Type};
use crate::macro_utils::InnerKind;

pub struct DbColumnMacros {
    pub field_def: FieldDef,
    pub range_query: Option<TokenStream>,
    pub stream_query_init: (TokenStream, TokenStream),
    pub table_definitions: Vec<TableDef>,
    pub struct_init: TokenStream,
    pub struct_init_with_query: TokenStream,
    pub struct_default_init: TokenStream,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_defs: Vec<FunctionDef>,
}

impl DbColumnMacros {
    pub fn new(field_def: FieldDef, indexing_type: IndexingType, entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type) -> DbColumnMacros {
        let column_name = &field_def.name.clone();
        let column_type = &field_def.tpe.clone();
        match indexing_type {
            IndexingType::Off => DbColumnMacros::plain(field_def, entity_name, pk_name, pk_type, column_name, column_type),
            IndexingType::On { dictionary: false, range } => {
                DbColumnMacros::index(field_def, entity_name, entity_type, pk_name, pk_type, column_name, column_type, range)
            }
            IndexingType::On { dictionary: true, range: false } => {
                DbColumnMacros::dictionary(field_def, entity_name, entity_type, pk_name, pk_type, column_name, column_type)
            }
            IndexingType::On { dictionary: true, range: true } => {
                panic!("Range indexing on dictionary columns is not supported")
            }
        }
    }

    pub fn plain(
        field_def: FieldDef,
        entity_name: &Ident,
        pk_name: &Ident,
        pk_type: &Type,
        column_name: &Ident,
        column_type: &Type,
    ) -> DbColumnMacros {
        let table_def = TableDef::plain_table_def(entity_name, column_name, column_type, pk_name, pk_type);
        DbColumnMacros {
            field_def,
            range_query: None,
            stream_query_init: query::stream_query_init(column_name, column_type),
            table_definitions: vec![table_def.clone()],
            struct_init: init::plain_init(column_name, &table_def.name),
            struct_init_with_query: init::plain_init_with_query(column_name, &table_def.name),
            struct_default_init: init::plain_default_init(column_name, column_type),
            store_statement: store::store_statement(pk_name, column_name, &table_def.name),
            store_many_statement: store::store_many_statement(pk_name, column_name, &table_def.name),
            delete_statement: delete::delete_statement(&table_def.name),
            delete_many_statement: delete::delete_many_statement(&table_def.name),
            function_defs: vec![],
        }
    }

    pub fn index(
        field_def: FieldDef,
        entity_name: &Ident,
        entity_type: &Type,
        pk_name: &Ident,
        pk_type: &Type,
        column_name: &Ident,
        column_type: &Type,
        range: bool,
    ) -> DbColumnMacros {
        let plain_table_def = TableDef::plain_table_def(entity_name, column_name, column_type, pk_name, pk_type);
        let index_table_def = TableDef::index_table_def(entity_name, column_name, column_type, pk_type);

        let mut function_defs: Vec<FunctionDef> = Vec::new();
        function_defs.push(get_by::get_by_index_def(entity_name, entity_type, column_name, column_type, &index_table_def.name));
        function_defs.push(stream_by::by_index_def(entity_name, entity_type, column_name, column_type, &index_table_def.name));
        function_defs.push(get_keys_by::by_index_def(
            entity_name,
            pk_name,
            pk_type,
            column_name,
            column_type,
            &index_table_def.name,
        ));
        function_defs.push(stream_keys_by::by_index_def(
            entity_name,
            pk_name,
            pk_type,
            column_name,
            column_type,
            &index_table_def.name,
        ));
        let entity_column_range_query = format_ident!("{}{}RangeQuery", entity_name.to_string(), column_name.to_string());
        let entity_column_range_query_ty = syn::parse_quote!(#entity_column_range_query);
        let mut range_query = None;

        if range {
            range_query = Some(quote! {
                #[derive(IntoParams, Serialize, Deserialize, Default)]
                pub struct #entity_column_range_query {
                    pub from: #column_type,
                    pub until: #column_type,
                }
                impl #entity_column_range_query {
                    pub fn sample() -> Self {
                        Self {
                            from: #column_type::default(),
                            until: #column_type::default().next()
                        }
                    }
                }
            });
            function_defs.push(stream_range_by::stream_range_by_index_def(
                entity_name,
                entity_type,
                column_name,
                column_type,
                &index_table_def.name,
                entity_column_range_query_ty,
            ));
            function_defs.push(range_by::by_index_def(
                entity_name,
                entity_type,
                column_name,
                column_type,
                &index_table_def.name,
            ));
        };

        DbColumnMacros {
            field_def,
            range_query,
            stream_query_init: query::stream_query_init(column_name, column_type),
            table_definitions: vec![plain_table_def.clone(), index_table_def.clone()],
            struct_init: init::index_init(column_name, &plain_table_def.name),
            struct_init_with_query: init::index_init_with_query(column_name, &plain_table_def.name),
            struct_default_init: init::index_default_init(column_name, column_type),
            store_statement: store::store_index_def(column_name, pk_name, &plain_table_def.name, &index_table_def.name),
            store_many_statement: store::store_many_index_def(column_name, pk_name, &plain_table_def.name, &index_table_def.name),
            delete_statement: delete::delete_index_statement(&plain_table_def.name, &index_table_def.name),
            delete_many_statement: delete::delete_many_index_statement(&plain_table_def.name, &index_table_def.name),
            function_defs,
        }
    }

    pub fn dictionary(
        field_def: FieldDef,
        entity_name: &Ident,
        entity_type: &Type,
        pk_name: &Ident,
        pk_type: &Type,
        column_name: &Ident,
        column_type: &Type,
    ) -> DbColumnMacros {
        let dict_index_table_def = TableDef::dict_index_table_def(entity_name, column_name, pk_type);
        let value_by_dict_pk_table_def = TableDef::value_by_dict_pk_table_def(entity_name, column_name, column_type, pk_type);
        let value_to_dict_pk_table_def = TableDef::value_to_dict_pk_table_def(entity_name, column_name, column_type, pk_type);
        let dict_pk_by_pk_table_def = TableDef::dict_pk_by_pk_table_def(entity_name, column_name, pk_name, pk_type);

        DbColumnMacros {
            field_def,
            range_query: None,
            stream_query_init: query::stream_query_init(column_name, column_type),
            table_definitions: vec![
                dict_index_table_def.clone(),
                value_by_dict_pk_table_def.clone(),
                value_to_dict_pk_table_def.clone(),
                dict_pk_by_pk_table_def.clone(),
            ],
            struct_init: init::dict_init(column_name, &dict_pk_by_pk_table_def.name, &value_by_dict_pk_table_def.name),
            struct_init_with_query: init::dict_init_with_query(
                column_name,
                &dict_pk_by_pk_table_def.name,
                &value_by_dict_pk_table_def.name,
            ),
            struct_default_init: init::dict_default_init(column_name, column_type),
            store_statement: store::store_dict_def(
                column_name,
                pk_name,
                &dict_pk_by_pk_table_def.name,
                &value_to_dict_pk_table_def.name,
                &value_by_dict_pk_table_def.name,
                &dict_index_table_def.name,
            ),
            store_many_statement: store::store_many_dict_def(
                column_name,
                pk_name,
                &dict_pk_by_pk_table_def.name,
                &value_to_dict_pk_table_def.name,
                &value_by_dict_pk_table_def.name,
                &dict_index_table_def.name,
            ),
            delete_statement: delete::delete_dict_statement(
                &dict_pk_by_pk_table_def.name,
                &value_to_dict_pk_table_def.name,
                &value_by_dict_pk_table_def.name,
                &dict_index_table_def.name,
            ),
            delete_many_statement: delete::delete_many_dict_statement(
                &dict_pk_by_pk_table_def.name,
                &value_to_dict_pk_table_def.name,
                &value_by_dict_pk_table_def.name,
                &dict_index_table_def.name,
            ),
            function_defs: vec![
                get_by::get_by_dict_def(
                    entity_name,
                    entity_type,
                    column_name,
                    column_type,
                    &value_to_dict_pk_table_def.name,
                    &dict_index_table_def.name,
                ),
                stream_by::by_dict_def(
                    entity_name,
                    entity_type,
                    column_name,
                    column_type,
                    &value_to_dict_pk_table_def.name,
                    &dict_index_table_def.name,
                ),
                get_keys_by::by_dict_def(
                    entity_name,
                    pk_name,
                    pk_type,
                    column_name,
                    column_type,
                    &value_to_dict_pk_table_def.name,
                    &dict_index_table_def.name,
                ),
                stream_keys_by::by_dict_def(
                    entity_name,
                    pk_name,
                    pk_type,
                    column_name,
                    column_type,
                    &value_to_dict_pk_table_def.name,
                    &dict_index_table_def.name,
                ),
            ],
        }
    }

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
}
