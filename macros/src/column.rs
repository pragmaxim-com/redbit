mod get_by;
mod range_by;
mod store;
mod delete;
mod init;

use crate::field_parser::{ColumnDef, Indexing, PkDef};
use crate::macro_utils;
use crate::rest::*;
use crate::table::TableDef;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{ItemStruct, Type};

pub struct DbColumnMacros {
    pub definition: ColumnDef,
    pub query: Option<TokenStream>,
    pub table_definitions: Vec<TableDef>,
    pub struct_init: TokenStream,
    pub struct_default_init: TokenStream,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_defs: Vec<FunctionDef>,
}

impl DbColumnMacros {
    pub fn new(column_def: ColumnDef, entity_name: &Ident, entity_type: &Type, pk_def: &PkDef) -> Result<DbColumnMacros, syn::Error> {
        let column_name = &column_def.field.name.clone();
        let column_type = &column_def.field.tpe.clone();
        let pk_name = &pk_def.field.name;
        let pk_type = &pk_def.field.tpe;
        match column_def.indexing {
            Indexing::Off =>
                Ok(DbColumnMacros::plain(column_def, entity_name, pk_name, pk_type, column_name, column_type)),
            Indexing::On { dictionary: false, range } =>
                Ok(DbColumnMacros::index(column_def, entity_name, entity_type, pk_name, pk_type, column_name, column_type, range)),
            Indexing::On { dictionary: true, range: false } =>
                Ok(DbColumnMacros::dictionary(column_def, entity_name, entity_type, pk_name, pk_type, column_name, column_type)),
            Indexing::On { dictionary: true, range: true } =>
                Err(syn::Error::new(column_name.span(), "Range indexing on dictionary columns is not supported"))
        }
    }

    pub fn plain(definition: ColumnDef, entity_name: &Ident, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type) -> DbColumnMacros {
        let table_def = TableDef::plain_table_def(entity_name, column_name, column_type, pk_name, pk_type);
        DbColumnMacros {
            definition,
            query: None,
            table_definitions: vec![table_def.clone()],
            struct_init: init::plain_init(column_name, &table_def.name),
            struct_default_init: init::plain_default_init(column_name, column_type),
            store_statement: store::store_statement(pk_name, column_name, &table_def.name),
            store_many_statement: store::store_many_statement(pk_name, column_name, &table_def.name),
            delete_statement: delete::delete_statement(&table_def.name),
            delete_many_statement: delete::delete_many_statement(&table_def.name),
            function_defs: vec![]
        }
    }

    pub fn index(definition: ColumnDef, entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type, range: bool) -> DbColumnMacros {
        let plain_table_def = TableDef::plain_table_def(entity_name, column_name, column_type, pk_name, pk_type);
        let index_table_def = TableDef::index_table_def(entity_name, column_name, column_type, pk_type);
        
        let mut function_defs: Vec<FunctionDef> = Vec::new();
        function_defs.push(get_by::get_by_index_def(entity_name, entity_type, column_name, column_type, &index_table_def.name));
        let entity_column_range_query = format_ident!("{}{}Range", entity_name.to_string(), column_name.to_string());
        let mut query = None;

        if range {
            query = Some(quote! {
                #[derive(utoipa::IntoParams, serde::Serialize, serde::Deserialize, Default)]
                pub struct #entity_column_range_query {
                    pub from: #column_type,
                    pub until: #column_type,
                }
                impl #entity_column_range_query {
                    pub fn sample() -> Vec<Self> {
                        vec![
                            #entity_column_range_query {
                                from: #column_type::default(),
                                until: #column_type::default()
                            }
                        ]
                    }
                }
            });
            function_defs.push(range_by::range_by_index_def(entity_name, entity_type, column_name, column_type, &index_table_def.name, &entity_column_range_query));
        };

        DbColumnMacros {
            definition,
            query,
            table_definitions: vec![plain_table_def.clone(), index_table_def.clone()],
            struct_init: init::index_init(column_name, &plain_table_def.name),
            struct_default_init: init::index_default_init(column_name, column_type),
            store_statement: store::store_index_def(column_name, pk_name, &plain_table_def.name, &index_table_def.name),
            store_many_statement: store::store_many_index_def(column_name, pk_name, &plain_table_def.name, &index_table_def.name),
            delete_statement: delete::delete_index_statement(&plain_table_def.name, &index_table_def.name),
            delete_many_statement: delete::delete_many_index_statement(&plain_table_def.name, &index_table_def.name),
            function_defs,
        }
    }

    pub fn dictionary(definition: ColumnDef, entity_name: &Ident, entity_type: &Type, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type) -> DbColumnMacros {
        let dict_index_table_def = TableDef::dict_index_table_def(entity_name, column_name, pk_type);
        let value_by_dict_pk_table_def = TableDef::value_by_dict_pk_table_def(entity_name, column_name, column_type, pk_type);
        let value_to_dict_pk_table_def = TableDef::value_to_dict_pk_table_def(entity_name, column_name, column_type, pk_type);
        let dict_pk_by_pk_table_def = TableDef::dict_pk_by_pk_table_def(entity_name, column_name, pk_name, pk_type);

        DbColumnMacros {
            definition,
            query: None,
            table_definitions: vec![
                dict_index_table_def.clone(),
                value_by_dict_pk_table_def.clone(),
                value_to_dict_pk_table_def.clone(),
                dict_pk_by_pk_table_def.clone()
            ],
            struct_init: init::dict_init_statement(column_name, &dict_pk_by_pk_table_def.name, &value_by_dict_pk_table_def.name),
            struct_default_init: init::dict_default_init(column_name, column_type),
            store_statement: store::store_dict_def(
                column_name,
                pk_name,
                &dict_pk_by_pk_table_def.name,
                &value_to_dict_pk_table_def.name,
                &value_by_dict_pk_table_def.name,
                &dict_index_table_def.name
            ),
            store_many_statement: store::store_many_dict_def(
                column_name,
                pk_name,
                &dict_pk_by_pk_table_def.name,
                &value_to_dict_pk_table_def.name,
                &value_by_dict_pk_table_def.name,
                &dict_index_table_def.name
            ),
            delete_statement: delete::delete_dict_statement(
                &dict_pk_by_pk_table_def.name,
                &value_to_dict_pk_table_def.name,
                &value_by_dict_pk_table_def.name,
                &dict_index_table_def.name
            ),
            delete_many_statement: delete::delete_many_dict_statement(
                &dict_pk_by_pk_table_def.name,
                &value_to_dict_pk_table_def.name,
                &value_by_dict_pk_table_def.name,
                &dict_index_table_def.name
            ),
            function_defs: vec![
                get_by::get_by_dict_def(
                    entity_name,
                    entity_type,
                    column_name,
                    column_type,
                    &value_to_dict_pk_table_def.name,
                    &dict_index_table_def.name
                )
            ]
        }
    }

    pub fn generate_index_impls(struct_ident: &Ident, input: &ItemStruct, inner_type: &Type) -> TokenStream {
        let serialization_code = if macro_utils::is_byte_array(inner_type) {
            quote! {
                if serializer.is_human_readable() {
                    serializer.serialize_str(&hex::encode(&self.0))
                } else {
                    self.0.serialize(serializer)
                }
            }
        } else {
            quote! {
                self.0.serialize(serializer)
            }
        };

        let deserialization_code = if macro_utils::is_byte_array(inner_type) {
            quote! {
                if deserializer.is_human_readable() {
                    let s = <&str>::deserialize(deserializer)?;
                    let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
                    if bytes.len() != std::mem::size_of::<#inner_type>() {
                        return Err(serde::de::Error::custom("Invalid length"));
                    }
                    let mut array = [0u8; std::mem::size_of::<#inner_type>()];
                    array.copy_from_slice(&bytes);
                    Ok(#struct_ident(array))
                } else {
                    let inner = <#inner_type>::deserialize(deserializer)?;
                    Ok(#struct_ident(inner))
                }
            }
        } else {
            quote! {
                let inner = <#inner_type>::deserialize(deserializer)?;
                Ok(#struct_ident(inner))
            }
        };

        let display_impl = if macro_utils::is_byte_array(inner_type) {
            quote! {
                impl std::fmt::Display for #struct_ident {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(f, "{}", hex::encode(&self.0))
                    }
                }
           }
        } else {
            quote! {
                impl std::fmt::Display for #struct_ident {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(f, "{}", self.0)
                    }
                }
            }
        };

        let serde = quote! {
            impl serde::Serialize for #struct_ident {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: serde::Serializer {
                    #serialization_code
                }
            }

            impl<'de> serde::Deserialize<'de> for #struct_ident {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: serde::Deserializer<'de> {
                    #deserialization_code
                }
            }
        };

        // Determine default value based on inner type
        let default_expr = match quote!(#inner_type).to_string().as_str() {
            "String" | "std :: string :: String" => {
                quote! { Self("a".to_string()) }
            },
            "&str" => {
                quote! { Self("a".into()) }
            },
            _ => {
                quote! { Self(Default::default()) }
            }
        };

        let default_impl = quote! {
            impl Default for #struct_ident {
                fn default() -> Self {
                    #default_expr
                }
            }
        };
        // RangeColumn impl based on inner type
        let next_impl = if macro_utils::is_integer(inner_type) {
            quote! {
                impl RangeColumn for #struct_ident {
                    fn next(&self) -> Self {
                        Self(self.0.wrapping_add(1))
                    }
                }
            }
        } else if macro_utils::is_string(inner_type) {
            quote! {
                impl RangeColumn for #struct_ident {
                    fn next(&self) -> Self {
                        let mut bytes = self.0.clone().into_bytes();
                        if let Some(last) = bytes.last_mut() {
                            *last = last.wrapping_add(1);
                        } else {
                            bytes.push(1);
                        }
                        Self(String::from_utf8(bytes).expect("Invalid UTF-8"))
                    }
                }
            }
        } else if macro_utils::is_byte_array(inner_type) {
            let len = macro_utils::get_array_len(inner_type).unwrap();
            quote! {
                impl RangeColumn for #struct_ident {
                    fn next(&self) -> Self {
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
                    }
                }
            }
        } else {
            quote! {
                compile_error!("RangeColumn not supported for this inner type");
            }
        };

        let expanded = quote! {
            #input
            #serde
            #default_impl
            #display_impl
            #next_impl
        };
        macro_utils::write_stream_and_return(expanded, struct_ident)
    }
}
