use proc_macro2::{Ident, TokenStream};
use crate::column::DbColumnMacros;
use crate::pk::DbPkMacros;
use crate::relationship::DbRelationshipMacros;
use crate::rest::FunctionDef;
use crate::table::TableDef;
use crate::transient::TransientMacros;

pub enum FieldMacros {
    Pk(DbPkMacros),
    Plain(DbColumnMacros),
    Relationship(DbRelationshipMacros),
    Transient(TransientMacros),
}

impl FieldMacros {
    pub fn field_name(&self) -> Ident {
        match self {
            FieldMacros::Pk(pk) => pk.field_def.name.clone(),
            FieldMacros::Plain(column) => column.field_def.name.clone(),
            FieldMacros::Relationship(relationship) => relationship.field_def.name.clone(),
            FieldMacros::Transient(transient) => transient.field_def.name.clone(),
        }
    }
    
    pub fn struct_init(&self) -> TokenStream {
        match self {
            FieldMacros::Pk(pk) => pk.struct_init.clone(),
            FieldMacros::Plain(column) => column.struct_init.clone(),
            FieldMacros::Relationship(relationship) => relationship.struct_init.clone(),
            FieldMacros::Transient(transient) => transient.struct_init.clone(),
        }
    }
    
    pub fn struct_init_with_query(&self) -> TokenStream {
        match self {
            FieldMacros::Pk(pk) => pk.struct_init_with_query.clone(),
            FieldMacros::Plain(column) => column.struct_init_with_query.clone(),
            FieldMacros::Relationship(relationship) => relationship.struct_init_with_query.clone(),
            FieldMacros::Transient(transient) => transient.struct_init_with_query.clone(),
        }
    }

    pub fn struct_default_init(&self) -> TokenStream {
        match self {
            FieldMacros::Pk(pk) => pk.struct_default_init.clone(),
            FieldMacros::Plain(column) => column.struct_default_init.clone(),
            FieldMacros::Relationship(relationship) => relationship.struct_default_init.clone(),
            FieldMacros::Transient(transient) => transient.struct_default_init.clone(),
        }
    }

    pub fn table_definitions(&self) -> Vec<TableDef> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.table_def.clone()],
            FieldMacros::Plain(column) => column.table_definitions.clone(),
            _ => vec![],
        }
    }

    pub fn range_queries(&self) -> Vec<TokenStream> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.range_query.clone()],
            FieldMacros::Plain(column) => column.range_query.clone().into_iter().collect(),
            _ => vec![],
        }
    }

    pub fn stream_queries(&self) -> Vec<(TokenStream, TokenStream)> {
        match self {
            FieldMacros::Plain(column) => vec![column.stream_query_init.clone()],
            _ => vec![],
        }
    }

    pub fn function_defs(&self) -> Vec<FunctionDef> {
        match self {
            FieldMacros::Pk(pk) => pk.function_defs.clone(),
            FieldMacros::Plain(column) => column.function_defs.clone(),
            FieldMacros::Relationship(relationship) => vec![relationship.function_def.clone()],
            _ => vec![],
        }
    }

    pub fn store_statements(&self) -> Vec<TokenStream> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.store_statement.clone()],
            FieldMacros::Plain(column) => vec![column.store_statement.clone()],
            FieldMacros::Relationship(relationship) => vec![relationship.store_statement.clone()],
            FieldMacros::Transient(_) => vec![],
        }
    }

    pub fn store_many_statements(&self) -> Vec<TokenStream> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.store_many_statement.clone()],
            FieldMacros::Plain(column) => vec![column.store_many_statement.clone()],
            FieldMacros::Relationship(relationship) => vec![relationship.store_many_statement.clone()],
            FieldMacros::Transient(_) => vec![],
        }
    }

    pub fn delete_statements(&self) -> Vec<TokenStream> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.delete_statement.clone()],
            FieldMacros::Plain(column) => vec![column.delete_statement.clone()],
            FieldMacros::Relationship(relationship) => vec![relationship.delete_statement.clone()],
            FieldMacros::Transient(_) => vec![],
        }
    }

    pub fn delete_many_statements(&self) -> Vec<TokenStream> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.delete_many_statement.clone()],
            FieldMacros::Plain(column) => vec![column.delete_many_statement.clone()],
            FieldMacros::Relationship(relationship) => vec![relationship.delete_many_statement.clone()],
            FieldMacros::Transient(_) => vec![],
        }
    }
}
