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
pub mod impls;

use crate::field_parser::{FieldDef, IndexingType};
use crate::rest::*;
use crate::table::TableDef;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;

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
}
