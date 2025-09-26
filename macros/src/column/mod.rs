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

use crate::entity;
use crate::entity::context;
use crate::entity::context::TxContextItem;
use crate::entity::query::{RangeQuery, FilterQueryItem};
use crate::field_parser::{EntityDef, FieldDef, IndexingType, OneToManyParentDef};
use crate::rest::*;
use crate::table::{DictTableDefs, IndexTableDefs, TableDef};
use proc_macro2::TokenStream;
use crate::entity::info::TableInfoItem;

pub struct DbColumnMacros {
    pub field_def: FieldDef,
    pub range_query: Option<RangeQuery>,
    pub table_plain_definitions: Vec<TableDef>,
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
    pub store_many_statement: TokenStream,
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
    ) -> DbColumnMacros {
        match indexing_type {
            IndexingType::Off { db_cache_weight, lru_cache_size}=> DbColumnMacros::plain(entity_def, col_field_def, db_cache_weight, lru_cache_size),
            IndexingType::On { dictionary: false, range, db_cache_weight, lru_cache_size } => {
                DbColumnMacros::index(entity_def, col_field_def, parent_def, range, db_cache_weight, lru_cache_size)
            }
            IndexingType::On { dictionary: true, range: false, db_cache_weight, lru_cache_size } => {
                DbColumnMacros::dictionary(entity_def, col_field_def, parent_def, db_cache_weight, lru_cache_size)
            }
            IndexingType::On { dictionary: true, range: true, db_cache_weight: _, lru_cache_size: _ } => {
                panic!("Range indexing on dictionary columns is not supported")
            }
        }
    }

    pub fn plain(
        entity_def: &EntityDef,
        col_def: &FieldDef,
        db_cache_weight: usize,
        lru_cache_size: usize
    ) -> DbColumnMacros {
        let column_name = &col_def.name.clone();
        let column_type = &col_def.tpe.clone();
        let pk_name = &entity_def.key_def.field_def().name;
        let table_def = TableDef::plain_table_def(entity_def, column_name, column_type, db_cache_weight, lru_cache_size);
        let table_definitions = vec![table_def.clone()];
        DbColumnMacros {
            field_def: col_def.clone(),
            range_query: None,
            filter_query_init: query::filter_query_init(column_name, column_type),
            tx_context_items: context::tx_context_items(&table_definitions),
            table_info_item: info::plain_table_info(column_name, &table_def),
            table_plain_definitions: table_definitions,
            table_index_definition: None,
            table_dict_definition: None,
            struct_init: init::plain_init(column_name, &table_def.var_name),
            struct_init_with_query: init::plain_init_with_query(column_name, &table_def.var_name),
            struct_default_init: init::default_init(column_name, column_type),
            struct_default_init_with_query: init::default_init_with_query(column_name, column_type),
            store_statement: store::store_statement(pk_name, column_name, &table_def.var_name),
            store_many_statement: store::store_statement(pk_name, column_name, &table_def.var_name),
            delete_statement: delete::delete_statement(&table_def.var_name),
            delete_many_statement: delete::delete_many_statement(&table_def.var_name),
            function_defs: vec![],
        }
    }

    pub fn index(
        entity_def: &EntityDef,
        col_field_def: &FieldDef,
        parent_def_opt: Option<OneToManyParentDef>,
        range: bool,
        db_cache_weight: usize,
        lru_cache_size: usize,
    ) -> DbColumnMacros {
        let column_name = &col_field_def.name.clone();
        let column_type = &col_field_def.tpe.clone();
        let pk_name = &entity_def.key_def.field_def().name;

        let index_tables = IndexTableDefs::new(entity_def, column_name, column_type, db_cache_weight, lru_cache_size);

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
            struct_default_init: init::default_init(column_name, column_type),
            struct_default_init_with_query: init::default_init_with_query(column_name, column_type),
            store_statement: store::store_index_def(column_name, &pk_name, &index_tables.var_name),
            store_many_statement: store::store_index_def(column_name, &pk_name, &index_tables.var_name),
            delete_statement: delete::delete_index_statement(&index_tables.var_name),
            delete_many_statement: delete::delete_many_index_statement(&index_tables.var_name),
            function_defs,
        }
    }

    pub fn dictionary(
        entity_def: &EntityDef,
        col_field_def: &FieldDef,
        parent_def_opt: Option<OneToManyParentDef>,
        db_cache_weight: usize,
        lru_cache_size: usize,
    ) -> DbColumnMacros {
        let column_name = &col_field_def.name.clone();
        let column_type = &col_field_def.tpe.clone();
        let pk_name = &entity_def.key_def.field_def().name;

        let dict_tables = DictTableDefs::new(&entity_def, column_name, column_type, db_cache_weight, lru_cache_size);

        let mut function_defs: Vec<FunctionDef> = Vec::new();

        function_defs.push(get_by::get_by_dict_def(entity_def, column_name, column_type, &dict_tables.var_name));
        function_defs.push(stream_by::by_dict_def(entity_def, column_name, column_type, &dict_tables.var_name));
        if let Some(parent_def) = parent_def_opt {
            function_defs.push(stream_parents_by::by_dict_def(entity_def, column_name, column_type, &dict_tables.var_name, &parent_def));
        }

        function_defs.push(get_keys_by::by_dict_def(entity_def, column_name, column_type, &dict_tables.var_name));
        function_defs.push(stream_keys_by::by_dict_def(entity_def, column_name, column_type, &dict_tables.var_name));

        let store_statement = store::store_dict_def(column_name, pk_name, &dict_tables.var_name);
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
            struct_default_init_with_query: init::default_init_with_query(column_name, column_type),
            struct_default_init: init::default_init(column_name, column_type),
            store_statement: store_statement.clone(),
            store_many_statement: store_statement,
            delete_statement: delete::delete_dict_statement(&dict_tables.var_name),
            delete_many_statement: delete::delete_many_dict_statement(&dict_tables.var_name),
            function_defs,
        }
    }
}

