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

use crate::field_parser::{FieldDef, IndexingType, OneToManyParentDef};
use crate::rest::*;
use crate::table::{DictTableDefs, StoreManyStmnt, TableDef};
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::Type;
use crate::entity;
use crate::entity::query::{RangeQuery, StreamQueryItem};

pub struct DbColumnMacros {
    pub field_def: FieldDef,
    pub range_query: Option<RangeQuery>,
    pub table_definitions: Vec<TableDef>,
    pub struct_init: TokenStream,
    pub stream_query_init: StreamQueryItem,
    pub struct_init_with_query: TokenStream,
    pub struct_default_init: TokenStream,
    pub struct_default_init_with_query: TokenStream,
    pub store_statement: TokenStream,
    pub store_many_statement: StoreManyStmnt,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_defs: Vec<FunctionDef>,
}

impl DbColumnMacros {

    pub fn new(
        col_field_def: &FieldDef,
        indexing_type: IndexingType,
        entity_name: &Ident,
        entity_type: &Type,
        pk_field_def: &FieldDef,
        stream_query_ty: &Type,
        parent_def: Option<OneToManyParentDef>,
    ) -> DbColumnMacros {
        let pk_name = &pk_field_def.name;
        let pk_type = &pk_field_def.tpe;
        match indexing_type {
            IndexingType::Off => DbColumnMacros::plain(col_field_def, entity_name, pk_name, pk_type),
            IndexingType::On { dictionary: false, range, cache_size: _ } => {
                DbColumnMacros::index(col_field_def, entity_name, entity_type, pk_field_def, stream_query_ty, parent_def, range)
            }
            IndexingType::On { dictionary: true, range: false, cache_size } => {
                DbColumnMacros::dictionary(col_field_def, entity_name, entity_type, pk_field_def, stream_query_ty, parent_def, cache_size)
            }
            IndexingType::On { dictionary: true, range: true, cache_size: _ } => {
                panic!("Range indexing on dictionary columns is not supported")
            }
        }
    }

    pub fn plain(
        field_def: &FieldDef,
        entity_name: &Ident,
        pk_name: &Ident,
        pk_type: &Type,
    ) -> DbColumnMacros {
        let column_name = &field_def.name.clone();
        let column_type = &field_def.tpe.clone();
        let table_def = TableDef::plain_table_def(entity_name, column_name, column_type, pk_name, pk_type);
        DbColumnMacros {
            field_def: field_def.clone(),
            range_query: None,
            stream_query_init: query::stream_query_init(column_name, column_type),
            table_definitions: vec![table_def.clone()],
            struct_init: init::plain_init(column_name, &table_def.name),
            struct_init_with_query: init::plain_init_with_query(column_name, &table_def.name),
            struct_default_init: init::default_init(column_name, column_type),
            struct_default_init_with_query: init::default_init_with_query(column_name, column_type),
            store_statement: store::store_statement(pk_name, column_name, &table_def.name),
            store_many_statement: store::store_many_statement(pk_name, column_name, &table_def.name),
            delete_statement: delete::delete_statement(&table_def.name),
            delete_many_statement: delete::delete_many_statement(&table_def.name),
            function_defs: vec![],
        }
    }

    pub fn index(
        col_field_def: &FieldDef,
        entity_name: &Ident,
        entity_type: &Type,
        pk_field_def: &FieldDef,
        stream_query_type: &Type,
        parent_def_opt: Option<OneToManyParentDef>,
        range: bool,
    ) -> DbColumnMacros {
        let column_name = &col_field_def.name.clone();
        let column_type = &col_field_def.tpe.clone();
        let pk_name = &pk_field_def.name;
        let pk_type = &pk_field_def.tpe;

        let plain_table_def = TableDef::plain_table_def(entity_name, column_name, column_type, pk_name, pk_type);
        let index_table_def = TableDef::index_table_def(entity_name, column_name, column_type, pk_type);

        let mut function_defs: Vec<FunctionDef> = Vec::new();
        function_defs.push(get_by::get_by_index_def(entity_name, entity_type, column_name, column_type, &index_table_def.name));
        function_defs.push(stream_by::by_index_def(entity_name, entity_type, column_name, column_type, pk_type, &index_table_def.name, stream_query_type));
        if let Some(parent_def) = parent_def_opt {
            function_defs.push(stream_parents_by::by_index_def(entity_name, column_name, column_type, pk_type, &index_table_def.name, &parent_def));
        }
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
        let mut range_query = None;

        if range {
            let rq = entity::query::col_range_query(entity_name, column_name, column_type);
            function_defs.push(stream_range_by::stream_range_by_index_def(
                entity_name,
                entity_type,
                col_field_def,
                pk_type,
                &index_table_def.name,
                &rq.ty,
                stream_query_type
            ));
            function_defs.push(range_by::by_index_def(
                entity_name,
                entity_type,
                column_name,
                column_type,
                &index_table_def.name,
            ));
            range_query = Some(rq);
        };

        DbColumnMacros {
            field_def: col_field_def.clone(),
            range_query,
            stream_query_init: query::stream_query_init(column_name, column_type),
            table_definitions: vec![plain_table_def.clone(), index_table_def.clone()],
            struct_init: init::index_init(column_name, &plain_table_def.name),
            struct_init_with_query: init::index_init_with_query(column_name, &plain_table_def.name),
            struct_default_init: init::default_init(column_name, column_type),
            struct_default_init_with_query: init::default_init_with_query(column_name, column_type),
            store_statement: store::store_index_def(column_name, pk_name, &plain_table_def.name, &index_table_def.name),
            store_many_statement: store::store_many_index_def(column_name, pk_name, &plain_table_def.name, &index_table_def.name),
            delete_statement: delete::delete_index_statement(&plain_table_def.name, &index_table_def.name),
            delete_many_statement: delete::delete_many_index_statement(&plain_table_def.name, &index_table_def.name),
            function_defs,
        }
    }

    pub fn dictionary(
        col_field_def: &FieldDef,
        entity_name: &Ident,
        entity_type: &Type,
        pk_field_def: &FieldDef,
        stream_query_type: &Type,
        parent_def_opt: Option<OneToManyParentDef>,
        cache_size: Option<usize>,
    ) -> DbColumnMacros {
        let column_name = &col_field_def.name.clone();
        let column_type = &col_field_def.tpe.clone();
        let pk_name = &pk_field_def.name;
        let pk_type = &pk_field_def.tpe;

        let dict_tables = DictTableDefs::new(entity_name, column_name, column_type, pk_name, pk_type, cache_size);

        let mut function_defs: Vec<FunctionDef> = Vec::new();

        function_defs.push(get_by::get_by_dict_def(entity_name, entity_type, column_name, column_type, &dict_tables));
        function_defs.push(stream_by::by_dict_def(entity_name, entity_type, column_name, column_type, pk_type, &dict_tables, stream_query_type));
        if let Some(parent_def) = parent_def_opt {
            function_defs.push(
                stream_parents_by::by_dict_def(entity_name, column_name, column_type, pk_type, &dict_tables, &parent_def)
            );
        }

        function_defs.push(get_keys_by::by_dict_def(entity_name, pk_name, pk_type, column_name, column_type, &dict_tables));
        function_defs.push(stream_keys_by::by_dict_def(entity_name, pk_name, pk_type, column_name, column_type, &dict_tables));

        DbColumnMacros {
            field_def: col_field_def.clone(),
            range_query: None,
            stream_query_init: query::stream_query_init(column_name, column_type),
            table_definitions: dict_tables.all_table_defs(),
            struct_init: init::dict_init(column_name, &dict_tables),
            struct_init_with_query: init::dict_init_with_query(column_name, &dict_tables),
            struct_default_init_with_query: init::default_init_with_query(column_name, column_type),
            struct_default_init: init::default_init(column_name, column_type),
            store_statement: store::store_dict_def(column_name, pk_name, &dict_tables),
            store_many_statement: store::store_many_dict_def(column_name, pk_name, &dict_tables),
            delete_statement: delete::delete_dict_statement(&dict_tables),
            delete_many_statement: delete::delete_many_dict_statement(&dict_tables),
            function_defs,
        }
    }
}

pub(crate) fn open_dict_tables(dict_table_defs: &DictTableDefs) -> (TokenStream, Ident, Ident, Ident, Ident) {
    let table_dict_pk_by_pk = &dict_table_defs.dict_pk_by_pk_table_def.name;
    let table_value_to_dict_pk = &dict_table_defs.value_to_dict_pk_table_def.name;
    let table_value_by_dict_pk = &dict_table_defs.value_by_dict_pk_table_def.name;
    let table_dict_index = &dict_table_defs.dict_index_table_def.name;

    let dict_pk_by_pk_var = Ident::new(&format!("{}_col_var", table_dict_pk_by_pk).to_lowercase(), table_dict_pk_by_pk.span());
    let value_to_dict_pk_var = Ident::new(&format!("{}_col_var", table_value_to_dict_pk).to_lowercase(), table_dict_pk_by_pk.span());
    let value_by_dict_pk_var = Ident::new(&format!("{}_col_var", table_value_by_dict_pk).to_lowercase(), table_dict_pk_by_pk.span());
    let dict_index_var = Ident::new(&format!("{}_col_var", table_dict_index).to_lowercase(), table_dict_pk_by_pk.span());

    let stream =
        quote! {
            let mut #dict_pk_by_pk_var       = tx.open_table(#table_dict_pk_by_pk)?;
            let mut #value_to_dict_pk_var    = tx.open_table(#table_value_to_dict_pk)?;
            let mut #value_by_dict_pk_var    = tx.open_table(#table_value_by_dict_pk)?;
            let mut #dict_index_var          = tx.open_multimap_table(#table_dict_index)?;
        };
    (stream, dict_pk_by_pk_var, value_to_dict_pk_var, value_by_dict_pk_var, dict_index_var)
}
