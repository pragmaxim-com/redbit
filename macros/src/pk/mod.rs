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

use crate::field_parser::{FieldDef, Multiplicity};
use crate::rest::FunctionDef;
use crate::table::TableDef;
use proc_macro2::{Ident, TokenStream};
use syn::Type;
use crate::entity;
use crate::entity::context;
use crate::entity::context::{TxContextItem, TxType};
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
    pub tx_context_item: TxContextItem,
    pub range_query: RangeQuery,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_defs: Vec<FunctionDef>,
}

impl DbPkMacros {
    pub fn new(entity_name: &Ident, entity_type: &Type, pk_field_def: &FieldDef, multiplicity: Option<Multiplicity>, stream_query_ty: &Type, no_columns: bool) -> Self {
        let pk_name = pk_field_def.name.clone();
        let pk_type = pk_field_def.tpe.clone();
        let table_def = TableDef::pk(entity_name, &pk_name, &pk_type);
        let range_query = entity::query::pk_range_query(entity_name, &pk_name, &pk_type);
        let write_tx_context_ty = context::entity_tx_context_type(entity_type, TxType::Write);
        let read_tx_context_ty = context::entity_tx_context_type(entity_type, TxType::Read);

        let mut function_defs: Vec<FunctionDef> = vec![
            get::fn_def(entity_name, entity_type, &pk_name, &pk_type, &read_tx_context_ty, &table_def.var_name),
            filter::fn_def(entity_name, entity_type, &pk_type, &read_tx_context_ty, &table_def.var_name, stream_query_ty, no_columns),
            take::fn_def(entity_name, entity_type, &read_tx_context_ty, &table_def.var_name),
            tail::fn_def(entity_name, entity_type, &read_tx_context_ty, &table_def.var_name),
            first::fn_def(entity_name, entity_type, &read_tx_context_ty, &table_def.var_name),
            last::fn_def(entity_name, entity_type, &read_tx_context_ty, &table_def.var_name),
            exists::fn_def(entity_name, &pk_name, &pk_type, &read_tx_context_ty, &table_def.var_name),
            range::fn_def(entity_name, entity_type, &pk_type, &read_tx_context_ty, &table_def.var_name, stream_query_ty, no_columns),
            stream_range::fn_def(entity_name, entity_type, pk_field_def, &read_tx_context_ty, &table_def.var_name, &range_query.ty, stream_query_ty, no_columns),
            pk_range::fn_def(entity_name, entity_type, &pk_name, &pk_type, &table_def.var_name, &write_tx_context_ty),
        ];

        if let Some(Multiplicity::OneToMany) = multiplicity {
            function_defs.push(parent_key::fn_def(entity_name, &pk_name, &pk_type));
        }

        let pk_init = init::pk_init(&pk_name);
        DbPkMacros {
            field_def: pk_field_def.clone(),
            table_def: table_def.clone(),
            struct_init: pk_init.clone(),
            struct_init_with_query: pk_init.clone(),
            struct_default_init: pk_init.clone(),
            struct_default_init_with_query: pk_init.clone(),
            tx_context_item: context::tx_context_item(&table_def),
            range_query,
            store_statement: store::store_statement(&pk_name, &table_def.var_name),
            store_many_statement: store::store_many_statement(&pk_name, &table_def.var_name),
            delete_statement: delete::delete_statement(&table_def.var_name),
            delete_many_statement: delete::delete_many_statement(&table_def.var_name),
            function_defs,
        }
    }
}
