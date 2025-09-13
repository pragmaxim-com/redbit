use crate::column::DbColumnMacros;
use crate::entity::context::{TxContextItem, TxType};
use crate::entity::query::{RangeQuery, StreamQueryItem};
use crate::entity::{context, query};
use crate::field_parser;
use crate::field_parser::{ColumnDef, FieldDef, KeyDef, Multiplicity, OneToManyParentDef};
use crate::pk::DbPkMacros;
use crate::relationship::DbRelationshipMacros;
use crate::rest::FunctionDef;
use crate::table::{DictTableDefs, TableDef};
use crate::transient::TransientMacros;
use proc_macro2::{Ident, TokenStream};
use syn::{ItemStruct, Type};

pub enum FieldMacros {
    Pk(DbPkMacros),
    Plain(DbColumnMacros),
    Relationship(DbRelationshipMacros),
    Transient(TransientMacros),
}

impl FieldMacros {
    pub fn new(
        item_struct: &ItemStruct,
        entity_ident: &Ident,
        entity_type: &Type,
        read_tx_context_ty: &Type,
        stream_query_ty: &Type,
    ) -> Result<(KeyDef, Option<OneToManyParentDef>, Vec<FieldMacros>), syn::Error> {
        let (key_def, field_macros) = field_parser::get_field_macros(item_struct)?;
        let one_to_many_parent_def =
            match key_def.clone() {
                KeyDef::Fk{ field_def: _, multiplicity: Multiplicity::OneToMany , parent_type: Some(parent_ty)} => Some(OneToManyParentDef {
                    tx_context_ty: context::entity_tx_context_type(&parent_ty, TxType::Read),
                    stream_query_ty: query::stream_query_type(&parent_ty),
                    parent_type: parent_ty.clone(),
                    parent_ident: match parent_ty {
                        Type::Path(p) => p.path.segments.last().unwrap().ident.clone(),
                        _ => panic!("Unsupported parent type"),
                    },
                }),
                _ => None
            };
        let field_def = key_def.field_def();
        let field_macros = field_macros.iter().map(|c| match c {
            ColumnDef::Key(KeyDef::Pk(field_def)) => {
                FieldMacros::Pk(DbPkMacros::new(entity_ident, entity_type, field_def, None, stream_query_ty, field_macros.len() == 1))
            },
            ColumnDef::Key(KeyDef::Fk{ field_def, multiplicity, parent_type: _}) => {
                FieldMacros::Pk(DbPkMacros::new(entity_ident, entity_type, field_def, Some(multiplicity.clone()), stream_query_ty, field_macros.len() == 1))
            },
            ColumnDef::Plain(field , indexing_type) => {
                FieldMacros::Plain(
                    DbColumnMacros::new(
                        field,
                        indexing_type.clone(),
                        entity_ident,
                        entity_type,
                        &field_def,
                        read_tx_context_ty,
                        stream_query_ty,
                        one_to_many_parent_def.clone()
                    ))
            },
            ColumnDef::Relationship(field, write_from, multiplicity) => {
                FieldMacros::Relationship(DbRelationshipMacros::new(field.clone(), multiplicity.clone(), entity_ident, &field_def.name, &field_def.tpe, write_from.clone()))
            }
            ColumnDef::Transient(field, read_from) => {
                FieldMacros::Transient(TransientMacros::new(field.clone(), read_from.clone()))
            }
        }).collect::<Vec<FieldMacros>>();
        Ok((key_def, one_to_many_parent_def, field_macros))
    }

    pub fn field_def(&self) -> FieldDef {
        match self {
            FieldMacros::Pk(pk) => pk.field_def.clone(),
            FieldMacros::Plain(column) => column.field_def.clone(),
            FieldMacros::Relationship(relationship) => relationship.field_def.clone(),
            FieldMacros::Transient(transient) => transient.field_def.clone(),
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

    pub fn struct_default_init_with_query(&self) -> TokenStream {
        match self {
            FieldMacros::Pk(pk) => pk.struct_default_init_with_query.clone(),
            FieldMacros::Plain(column) => column.struct_default_init_with_query.clone(),
            FieldMacros::Relationship(relationship) => relationship.struct_default_init_with_query.clone(),
            FieldMacros::Transient(transient) => transient.struct_default_init_with_query.clone(),
        }
    }

    pub fn table_definitions(&self) -> Vec<TableDef> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.table_def.clone()],
            FieldMacros::Plain(column) => column.table_plain_definitions.clone(),
            _ => vec![],
        }
    }

    pub fn dict_table_definitions(&self) -> Option<DictTableDefs> {
        match self {
            FieldMacros::Plain(column) => column.table_dict_definition.clone(),
            _ => None,
        }
    }

    pub fn range_queries(&self) -> Vec<RangeQuery> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.range_query.clone()],
            FieldMacros::Plain(column) => column.range_query.clone().into_iter().collect(),
            _ => vec![],
        }
    }

    pub fn stream_queries(&self) -> Vec<StreamQueryItem> {
        match self {
            FieldMacros::Plain(column) => vec![column.stream_query_init.clone()],
            FieldMacros::Relationship(rel) => vec![rel.stream_query_init.clone()],
            _ => vec![],
        }
    }

    pub fn tx_context_items(&self) -> Vec<TxContextItem> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.tx_context_item.clone()],
            FieldMacros::Plain(column) => column.tx_context_items.clone(),
            FieldMacros::Relationship(rel) => vec![rel.tx_context_item.clone()],
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
