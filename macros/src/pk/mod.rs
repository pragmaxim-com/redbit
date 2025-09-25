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
mod init;
pub mod pointer_impls;
pub mod root_impls;

use crate::entity;
use crate::entity::context;
use crate::entity::context::TxContextItem;
use crate::entity::query::RangeQuery;
use crate::field_parser::{EntityDef, FieldDef, Multiplicity};
use crate::rest::FunctionDef;
use crate::table::TableDef;
use proc_macro2::TokenStream;

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
    pub tx_context_item: TxContextItem,
    pub range_query: RangeQuery,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_defs: Vec<FunctionDef>,
}

impl DbPkMacros {
    pub fn new(entity_def: &EntityDef, multiplicity: Option<Multiplicity>, no_columns: bool, db_cache_weight: usize) -> Self {
        let table_def = TableDef::pk(entity_def, db_cache_weight);
        let range_query = entity::query::pk_range_query(entity_def);

        let mut function_defs: Vec<FunctionDef> = vec![
            get::fn_def(entity_def, &table_def.var_name),
            filter::fn_def(entity_def, &table_def.var_name, no_columns),
            take::fn_def(entity_def, &table_def.var_name),
            tail::fn_def(entity_def, &table_def.var_name),
            first::fn_def(entity_def, &table_def.var_name),
            last::fn_def(entity_def, &table_def.var_name),
            exists::fn_def(entity_def, &table_def.var_name),
            range::fn_def(entity_def, &table_def.var_name, no_columns),
            stream_range::fn_def(entity_def, &table_def.var_name, &range_query.ty, no_columns),
            pk_range::fn_def(entity_def, &table_def.var_name),
        ];

        if let Some(Multiplicity::OneToMany) = multiplicity {
            function_defs.push(parent_key::fn_def(entity_def));
        }

        let pk_init = init::pk_init(&entity_def.key_def.field_def().name);
        DbPkMacros {
            field_def: entity_def.key_def.field_def().clone(),
            table_def: table_def.clone(),
            struct_init: pk_init.clone(),
            struct_init_with_query: pk_init.clone(),
            struct_default_init: pk_init.clone(),
            struct_default_init_with_query: pk_init.clone(),
            tx_context_item: context::tx_context_plain_item(&table_def),
            range_query,
            store_statement: store::store_statement(&entity_def.key_def.field_def().name, &table_def.var_name),
            store_many_statement: store::store_many_statement(&entity_def.key_def.field_def().name, &table_def.var_name),
            delete_statement: delete::delete_statement(&table_def.var_name),
            delete_many_statement: delete::delete_many_statement(&table_def.var_name),
            function_defs,
        }
    }
}
