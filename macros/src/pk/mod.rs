mod exists;
mod get;
mod tail;
mod take;
mod first;
mod filter;
mod last;
mod range;
mod stream_range;
mod pk_range;
mod store;
mod delete;
mod parent_key;
mod limit;
mod init;
pub mod pointer_impls;
pub mod root_impls;

use crate::field_parser::{FieldDef, Multiplicity};
use crate::rest::FunctionDef;
use crate::table::TableDef;
use proc_macro2::{Ident, TokenStream};
use syn::Type;
use crate::entity;
use crate::entity::query::RangeQuery;

pub enum PointerType {
    Root,
    Child,
}

pub struct DbPkMacros {
    pub field_def: FieldDef,
    pub table_def: TableDef,
    pub struct_init: TokenStream,
    pub struct_init_with_query: TokenStream,
    pub struct_default_init: TokenStream,
    pub struct_default_init_with_query: TokenStream,
    pub range_query: RangeQuery,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_defs: Vec<FunctionDef>,
}

impl DbPkMacros {
    pub fn new(entity_name: &Ident, entity_type: &Type, field_def: FieldDef, multiplicity: Option<Multiplicity>, stream_query_ty: &Type) -> Self {
        let pk_name = field_def.name.clone();
        let pk_type = field_def.tpe.clone();
        let table_def = TableDef::pk(entity_name, &pk_name, &pk_type);

        let mut function_defs: Vec<FunctionDef> = Vec::new();
        function_defs.push(get::fn_def(entity_name, entity_type, &pk_name, &pk_type, &table_def.name));
        function_defs.push(filter::fn_def(entity_name, entity_type, &pk_type, &table_def.name, stream_query_ty));
        function_defs.push(take::fn_def(entity_name, entity_type, &table_def.name));
        function_defs.push(tail::fn_def(entity_name, entity_type, &table_def.name));
        function_defs.push(first::fn_def(entity_name, entity_type, &table_def.name));
        function_defs.push(last::fn_def(entity_name, entity_type, &table_def.name));
        function_defs.push(limit::limit_fn_def(entity_name, entity_type));
        function_defs.push(exists::fn_def(entity_name, &pk_name, &pk_type, &table_def.name));

        match multiplicity {
            Some(Multiplicity::OneToMany) => {
                function_defs.push(parent_key::fn_def(entity_name, &pk_name, &pk_type));
            }
            _ => {}
        };

        let range_query = entity::query::pk_range_query(entity_name, &pk_name, &pk_type);
        function_defs.push(range::fn_def(entity_name, entity_type, &pk_type, &table_def.name, stream_query_ty));
        function_defs.push(stream_range::fn_def(entity_name, entity_type, &pk_name, &pk_type, &table_def.name, &range_query.ty, stream_query_ty));
        function_defs.push(pk_range::fn_def(entity_name, entity_type, &pk_name, &pk_type, &table_def.name));

        DbPkMacros {
            field_def,
            table_def: table_def.clone(),
            struct_init: init::pk_init(&pk_name),
            struct_init_with_query: init::pk_init_with_query(&pk_name),
            struct_default_init: init::pk_default_init(&pk_name),
            struct_default_init_with_query: init::pk_init_with_query(&pk_name),
            range_query,
            store_statement: store::store_statement(&pk_name, &table_def.name),
            store_many_statement: store::store_many_statement(&pk_name, &table_def.name),
            delete_statement: delete::delete_statement(&table_def.name),
            delete_many_statement: delete::delete_many_statement(&table_def.name),
            function_defs,
        }
    }
}
