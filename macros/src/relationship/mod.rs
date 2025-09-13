mod get;
mod init;
mod store;
mod delete;
mod query;
mod context;

use crate::field_parser::*;
use crate::rest::FunctionDef;
use proc_macro2::{Ident, TokenStream};
use syn::Type;
use crate::entity;
use crate::entity::context::{TxContextItem, TxType};
use crate::entity::query::StreamQueryItem;

pub struct DbRelationshipMacros {
    pub field_def: FieldDef,
    pub struct_init: TokenStream,
    pub stream_query_init: StreamQueryItem,
    pub tx_context_item: TxContextItem,
    pub struct_init_with_query: TokenStream,
    pub struct_default_init: TokenStream,
    pub struct_default_init_with_query: TokenStream,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_def: FunctionDef,
}

impl DbRelationshipMacros {
    pub fn new(field_def: FieldDef, multiplicity: Multiplicity, entity_ident: &Ident, pk_name: &Ident, pk_type: &Type, write_from: Option<WriteFrom>) -> DbRelationshipMacros {
        let child_name = &field_def.name; // e.g., "transactions"
        let child_type = &field_def.tpe; // e.g., the type `Transaction` from Vec<Transaction>
        let child_stream_query_type = entity::query::stream_query_type(child_type);
        let write_child_tx_context_type = entity::context::entity_tx_context_type(child_type, TxType::Write);
        let read_child_tx_context_type = entity::context::entity_tx_context_type(child_type, TxType::Read);
        match multiplicity {
            Multiplicity::OneToOne => {
                DbRelationshipMacros {
                    field_def: field_def.clone(),
                    struct_init: init::one2one_relation_init(child_name, child_type),
                    stream_query_init: query::stream_query_init(child_name, &child_stream_query_type),
                    tx_context_item: context::tx_context_item(child_name, &write_child_tx_context_type, &read_child_tx_context_type),
                    struct_init_with_query: init::one2one_relation_init_with_query(child_name, child_type),
                    struct_default_init: init::one2one_relation_default_init(child_name, child_type),
                    struct_default_init_with_query: init::one2one_relation_default_init_with_query(child_name, child_type),
                    store_statement: store::one2one_store_def(child_name, child_type),
                    store_many_statement: store::one2one_store_many_def(child_name, child_type),
                    delete_statement: delete::one2one_delete_def(child_name, child_type),
                    delete_many_statement: delete::one2one_delete_many_def(child_name, child_type),
                    function_def: get::one2one_def(entity_ident, child_name, child_type, pk_name, pk_type, &read_child_tx_context_type)
                }
            }
            Multiplicity::OneToOption => {
                DbRelationshipMacros {
                    field_def: field_def.clone(),
                    struct_init: init::one2opt_relation_init(child_name, child_type),
                    stream_query_init: query::stream_query_init(child_name, &child_stream_query_type),
                    tx_context_item: context::tx_context_item(child_name, &write_child_tx_context_type, &read_child_tx_context_type),
                    struct_init_with_query: init::one2opt_relation_init_with_query(child_name, child_type),
                    struct_default_init: init::one2opt_relation_default_init(child_name, child_type),
                    struct_default_init_with_query: init::one2opt_relation_default_init_with_query(child_name, child_type),
                    store_statement: store::one2opt_store_def(child_name, child_type),
                    store_many_statement: store::one2opt_store_many_def(child_name, child_type),
                    delete_statement: delete::one2opt_delete_def(child_name, child_type),
                    delete_many_statement: delete::one2opt_delete_many_def(child_name, child_type),
                    function_def: get::one2opt_def(entity_ident, child_name, child_type, pk_name, pk_type, &read_child_tx_context_type)
                }
            }
            Multiplicity::OneToMany => {
                let store_statement = store::one2many_store_def(child_name, child_type, pk_name, write_from.clone());
                DbRelationshipMacros {
                    field_def: field_def.clone(),
                    struct_init: init::one2many_relation_init(child_name, child_type),
                    stream_query_init: query::stream_query_init(child_name, &child_stream_query_type),
                    tx_context_item: context::tx_context_item(child_name, &write_child_tx_context_type, &read_child_tx_context_type),
                    struct_init_with_query: init::one2many_relation_init_with_query(child_name, child_type),
                    struct_default_init: init::one2many_relation_default_init(child_name, child_type),
                    struct_default_init_with_query: init::one2many_relation_default_init_with_query(child_name, child_type),
                    store_statement: store_statement.clone(),
                    store_many_statement: store_statement,
                    delete_statement: delete::one2many_delete_def(child_name, child_type),
                    delete_many_statement: delete::one2many_delete_many_def(child_name, child_type),
                    function_def: get::one2many_def(entity_ident, child_name, child_type, pk_name, pk_type, &read_child_tx_context_type)
                }
            }
        }
    }
}
