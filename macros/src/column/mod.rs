mod delete;
mod stream_by;
mod stream_parents_by;
mod init;
mod stream_range_by;
mod store;
mod stream_keys_by;
mod query;
mod range_by;
mod get_by;
mod get_keys_by;
pub mod column_impls;
pub mod column_codec;
pub mod info;
pub mod transient;

use crate::entity;
use crate::entity::context;
use crate::entity::context::TxContextItem;
use crate::entity::query::{FilterQueryItem, RangeQuery};
use crate::field_parser::{ColumnProps, EntityDef, FieldDef, IndexingType, OneToManyParentDef, Used};
use crate::rest::*;
use crate::table::{DictTableDefs, IndexTableDefs, PlainTableDef, TableDef};
use proc_macro2::TokenStream;
use crate::entity::info::TableInfoItem;

pub struct DbColumnMacros {
    pub field_def: FieldDef,
    pub range_query: Option<RangeQuery>,
    pub table_plain_definitions: Vec<PlainTableDef>,
    pub table_index_definition: Option<IndexTableDefs>,
    pub table_dict_definition: Option<DictTableDefs>,
    pub struct_init: TokenStream,
    pub filter_query_init: FilterQueryItem,
    pub tx_context_items: Vec<TxContextItem>,
    pub table_info_item: TableInfoItem,
    pub struct_init_with_query: TokenStream,
    pub struct_default_init: TokenStream,
    pub struct_default_init_with_query: TokenStream,
    pub store_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_defs: Vec<FunctionDef>,
}

impl DbColumnMacros {

    pub fn new(
        entity_def: &EntityDef,
        col_field_def: &FieldDef,
        indexing_type: IndexingType,
        parent_def: Option<OneToManyParentDef>,
        used: Option<Used>,
        is_pointer: bool,
    ) -> DbColumnMacros {
        match indexing_type {
            IndexingType::Off(column_props) => {
                DbColumnMacros::plain(entity_def, col_field_def, column_props, used, is_pointer)
            },
            IndexingType::Index(column_props) => {
                DbColumnMacros::index(entity_def, col_field_def, parent_def, false, column_props, used, is_pointer)
            }
            IndexingType::Range(column_props) => {
                DbColumnMacros::index(entity_def, col_field_def, parent_def, true, column_props, used, is_pointer)
            }
            IndexingType::Dict(column_props) => {
                DbColumnMacros::dictionary(entity_def, col_field_def, parent_def, column_props, used, is_pointer)
            }
        }
    }

    pub fn plain(entity_def: &EntityDef, col_def: &FieldDef, column_props: ColumnProps, used: Option<Used>, is_pointer: bool) -> DbColumnMacros {
        let column_name = &col_def.name.clone();
        let column_type = &col_def.tpe.clone();
        let pk_name = &entity_def.key_def.field_def().name;
        let table_def = TableDef::plain_table_def(entity_def, column_name, column_type);
        let plain_table_def = PlainTableDef::new(table_def, column_props, false);
        DbColumnMacros {
            field_def: col_def.clone(),
            range_query: None,
            filter_query_init: query::filter_query_init(column_name, column_type),
            tx_context_items: vec![context::tx_context_plain_item(&plain_table_def)],
            table_info_item: info::plain_table_info(column_name, &plain_table_def),
            table_plain_definitions: vec![plain_table_def.clone()],
            table_index_definition: None,
            table_dict_definition: None,
            struct_init: init::plain_init(column_name, &plain_table_def.var_name),
            struct_init_with_query: init::plain_init_with_query(column_name, &plain_table_def.var_name),
            struct_default_init: init::default_init(column_name, column_type, is_pointer),
            struct_default_init_with_query: init::default_init_with_query(column_name, column_type, is_pointer),
            store_statement: store::store_statement(pk_name, column_name, &plain_table_def.var_name, used),
            delete_statement: delete::delete_statement(&plain_table_def.var_name),
            delete_many_statement: delete::delete_many_statement(&plain_table_def.var_name),
            function_defs: vec![],
        }
    }

    pub fn index(
        entity_def: &EntityDef,
        col_field_def: &FieldDef,
        parent_def_opt: Option<OneToManyParentDef>,
        range: bool,
        column_props: ColumnProps,
        used: Option<Used>,
        is_pointer: bool,
    ) -> DbColumnMacros {
        let column_name = &col_field_def.name.clone();
        let column_type = &col_field_def.tpe.clone();
        let pk_name = &entity_def.key_def.field_def().name;

        let index_tables = IndexTableDefs::new(entity_def, column_name, column_type, column_props);

        let mut function_defs: Vec<FunctionDef> = Vec::new();
        function_defs.push(get_by::get_by_index_def(entity_def, column_name, column_type, &index_tables.var_name));
        function_defs.push(stream_by::by_index_def(entity_def, column_name, column_type, &index_tables.var_name));
        if let Some(parent_def) = parent_def_opt {
            function_defs.push(stream_parents_by::by_index_def(entity_def, column_name, column_type, &index_tables.var_name, &parent_def));
        }
        function_defs.push(get_keys_by::by_index_def(entity_def, column_name, column_type, &index_tables.var_name));
        function_defs.push(stream_keys_by::by_index_def(entity_def, column_name, column_type, &index_tables.var_name));
        let mut range_query = None;

        if range {
            let rq = entity::query::col_range_query(&entity_def.entity_name, column_name, column_type);
            function_defs.push(stream_range_by::stream_range_by_index_def(entity_def, column_name, column_type, &index_tables.var_name, &rq.ty));
            function_defs.push(range_by::by_index_def(entity_def, column_name, column_type, &index_tables.var_name));
            range_query = Some(rq);
        };

        DbColumnMacros {
            field_def: col_field_def.clone(),
            range_query,
            filter_query_init: query::filter_query_init(column_name, column_type),
            tx_context_items: vec![context::tx_context_index_item(&index_tables)],
            table_info_item: info::index_table_info(column_name, &index_tables),
            table_plain_definitions: vec![],
            table_index_definition: Some(index_tables.clone()),
            table_dict_definition: None,
            struct_init: init::index_init(column_name, &index_tables.var_name),
            struct_init_with_query: init::index_init_with_query(column_name, &index_tables.var_name),
            struct_default_init: init::default_init(column_name, column_type, is_pointer),
            struct_default_init_with_query: init::default_init_with_query(column_name, column_type, is_pointer),
            store_statement: store::store_index_def(column_name, &pk_name, &index_tables.var_name, used),
            delete_statement: delete::delete_index_statement(&index_tables.var_name),
            delete_many_statement: delete::delete_many_index_statement(&index_tables.var_name),
            function_defs,
        }
    }

    pub fn dictionary(
        entity_def: &EntityDef,
        col_field_def: &FieldDef,
        parent_def_opt: Option<OneToManyParentDef>,
        column_props: ColumnProps,
        used: Option<Used>,
        is_pointer: bool,
    ) -> DbColumnMacros {
        let column_name = &col_field_def.name.clone();
        let column_type = &col_field_def.tpe.clone();
        let pk_name = &entity_def.key_def.field_def().name;

        let dict_tables = DictTableDefs::new(&entity_def, column_name, column_type, column_props);

        let mut function_defs: Vec<FunctionDef> = Vec::new();

        function_defs.push(get_by::get_by_dict_def(entity_def, column_name, column_type, &dict_tables.var_name));
        function_defs.push(stream_by::by_dict_def(entity_def, column_name, column_type, &dict_tables.var_name));
        if let Some(parent_def) = parent_def_opt {
            function_defs.push(stream_parents_by::by_dict_def(entity_def, column_name, column_type, &dict_tables.var_name, &parent_def));
        }

        function_defs.push(get_keys_by::by_dict_def(entity_def, column_name, column_type, &dict_tables.var_name));
        function_defs.push(stream_keys_by::by_dict_def(entity_def, column_name, column_type, &dict_tables.var_name));

        let store_statement = store::store_dict_def(column_name, pk_name, &dict_tables.var_name, used);
        DbColumnMacros {
            field_def: col_field_def.clone(),
            range_query: None,
            filter_query_init: query::filter_query_init(column_name, column_type),
            tx_context_items: vec![context::tx_context_dict_item(&dict_tables)],
            table_info_item: info::dict_table_info(&dict_tables, column_name),
            table_plain_definitions: Vec::new(),
            table_index_definition: None,
            table_dict_definition: Some(dict_tables.clone()),
            struct_init: init::dict_init(column_name, &dict_tables.var_name),
            struct_init_with_query: init::dict_init_with_query(column_name, &dict_tables.var_name),
            struct_default_init_with_query: init::default_init_with_query(column_name, column_type, is_pointer),
            struct_default_init: init::default_init(column_name, column_type, is_pointer),
            store_statement: store_statement.clone(),
            delete_statement: delete::delete_dict_statement(&dict_tables.var_name),
            delete_many_statement: delete::delete_many_dict_statement(&dict_tables.var_name),
            function_defs,
        }
    }
}

