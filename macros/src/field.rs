use crate::column::transient::TransientMacros;
use crate::column::DbColumnMacros;
use crate::entity::context::{TxContextItem, TxType};
use crate::entity::info::TableInfoItem;
use crate::entity::query::{FilterQueryItem, RangeQuery};
use crate::entity::{context, query};
use crate::field_parser;
use crate::field_parser::{ColumnDef, EntityDef, FieldDef, KeyDef, Multiplicity, OneToManyParentDef};
use crate::pk::DbPkMacros;
use crate::relationship::transient::TransientRelationshipMacros;
use crate::relationship::{DbRelationshipMacros, StoreStatement};
use crate::rest::FunctionDef;
use crate::table::{DictTableDefs, IndexTableDefs, PlainTableDef};
use proc_macro2::{Ident, TokenStream};
use syn::{ItemStruct, Type};

pub enum FieldMacros {
    Pk(DbPkMacros),
    Plain(DbColumnMacros),
    Relationship(DbRelationshipMacros),
    TransientRel(TransientRelationshipMacros),
    Transient(TransientMacros),
}

impl FieldMacros {
    pub fn new(
        item_struct: &ItemStruct,
        entity_name: &Ident,
        entity_type: Type
    ) -> Result<(EntityDef, Option<OneToManyParentDef>, Vec<FieldMacros>), syn::Error> {
        let (key_def, col_defs) = field_parser::get_field_macros(item_struct)?;
        let one_to_many_parent_def =
            match key_def.clone() {
                KeyDef::Fk { field_def: _, multiplicity: Multiplicity::OneToMany , parent_type: Some(parent_ty), column_props: _} => Some(OneToManyParentDef {
                    tx_context_ty: context::entity_tx_context_type(&parent_ty, TxType::Read),
                    stream_query_ty: query::filter_query_type(&parent_ty),
                    parent_type: parent_ty.clone(),
                    parent_ident: match parent_ty {
                        Type::Path(p) => p.path.segments.last().unwrap().ident.clone(),
                        _ => panic!("Unsupported parent type"),
                    },
                }),
                _ => None
            };
        let entity_def= EntityDef::new(key_def.clone(), entity_name.clone(), entity_type.clone());
        let field_macros = col_defs.iter().map(|c| match c {
            ColumnDef::Key(KeyDef::Pk { field_def: _, column_props}) => {
                FieldMacros::Pk(DbPkMacros::new(&entity_def, None, col_defs.len() == 1, column_props.clone()))
            },
            ColumnDef::Key(KeyDef::Fk { field_def: _, multiplicity, parent_type: _, column_props}) => {
                FieldMacros::Pk(DbPkMacros::new(&entity_def, Some(multiplicity.clone()), col_defs.len() == 1, column_props.clone()))
            },
            ColumnDef::Plain(field, indexing_type, used_by) => {
                FieldMacros::Plain(
                    DbColumnMacros::new(
                        &entity_def,
                        field,
                        indexing_type.clone(),
                        one_to_many_parent_def.clone(),
                        used_by.clone()
                    )
                )
            },
            ColumnDef::TransientRel(field, read_from) => {
                FieldMacros::TransientRel(TransientRelationshipMacros::new(field.clone(), read_from.clone()))
            }
            ColumnDef::Relationship(field, write_from_using, _, multiplicity) => {
                FieldMacros::Relationship(DbRelationshipMacros::new(&entity_def, field.clone(), multiplicity.clone(), write_from_using.clone()))
            }
            ColumnDef::Transient(field) => {
                FieldMacros::Transient(TransientMacros::new(field.clone()))
            }
        }).collect::<Vec<FieldMacros>>();
        Ok((entity_def, one_to_many_parent_def, field_macros))
    }

    pub fn field_def(&self) -> FieldDef {
        match self {
            FieldMacros::Pk(pk) => pk.field_def.clone(),
            FieldMacros::Plain(column) => column.field_def.clone(),
            FieldMacros::Relationship(relationship) => relationship.field_def.clone(),
            FieldMacros::Transient(transient) => transient.field_def.clone(),
            FieldMacros::TransientRel(transient_rel) => transient_rel.field_def.clone(),
        }
    }
    
    pub fn struct_init(&self) -> TokenStream {
        match self {
            FieldMacros::Pk(pk) => pk.struct_init.clone(),
            FieldMacros::Plain(column) => column.struct_init.clone(),
            FieldMacros::Relationship(relationship) => relationship.struct_init.clone(),
            FieldMacros::Transient(transient) => transient.struct_init.clone(),
            FieldMacros::TransientRel(transient_rel) => transient_rel.struct_init.clone(),
        }
    }
    
    pub fn struct_init_with_query(&self) -> TokenStream {
        match self {
            FieldMacros::Pk(pk) => pk.struct_init_with_query.clone(),
            FieldMacros::Plain(column) => column.struct_init_with_query.clone(),
            FieldMacros::Relationship(relationship) => relationship.struct_init_with_query.clone(),
            FieldMacros::Transient(transient) => transient.struct_init_with_query.clone(),
            FieldMacros::TransientRel(transient_rel) => transient_rel.struct_init_with_query.clone(),
        }
    }

    pub fn struct_default_init(&self) -> TokenStream {
        match self {
            FieldMacros::Pk(pk) => pk.struct_default_init.clone(),
            FieldMacros::Plain(column) => column.struct_default_init.clone(),
            FieldMacros::Relationship(relationship) => relationship.struct_default_init.clone(),
            FieldMacros::Transient(transient) => transient.struct_default_init.clone(),
            FieldMacros::TransientRel(transient_rel) => transient_rel.struct_default_init.clone(),
        }
    }

    pub fn struct_default_init_with_query(&self) -> TokenStream {
        match self {
            FieldMacros::Pk(pk) => pk.struct_default_init_with_query.clone(),
            FieldMacros::Plain(column) => column.struct_default_init_with_query.clone(),
            FieldMacros::Relationship(relationship) => relationship.struct_default_init_with_query.clone(),
            FieldMacros::Transient(transient) => transient.struct_default_init_with_query.clone(),
            FieldMacros::TransientRel(transient_rel) => transient_rel.struct_default_init_with_query.clone(),
        }
    }

    pub fn plain_table_definitions(&self) -> Vec<PlainTableDef> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.plain_table_def.clone()],
            FieldMacros::Plain(column) => column.table_plain_definitions.clone(),
            _ => vec![],
        }
    }

    pub fn index_table_definitions(&self) -> Option<IndexTableDefs> {
        match self {
            FieldMacros::Plain(column) => column.table_index_definition.clone(),
            _ => None,
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

    pub fn stream_queries(&self) -> Vec<FilterQueryItem> {
        match self {
            FieldMacros::Plain(column) => vec![column.filter_query_init.clone()],
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

    pub fn table_info_items(&self) -> Vec<TableInfoItem> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.table_info_item.clone()],
            FieldMacros::Plain(column) => vec![column.table_info_item.clone()],
            FieldMacros::Relationship(rel) => vec![rel.table_info_item.clone()],
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

    pub fn store_statements(&self) -> Vec<StoreStatement> {
        match self {
            FieldMacros::Pk(pk) => vec![StoreStatement::Plain(pk.store_statement.clone())],
            FieldMacros::Plain(column) => vec![StoreStatement::Plain(column.store_statement.clone())],
            FieldMacros::Relationship(relationship) => vec![relationship.store_statement.clone()],
            _ => vec![],
        }
    }

    pub fn delete_statements(&self) -> Vec<TokenStream> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.delete_statement.clone()],
            FieldMacros::Plain(column) => vec![column.delete_statement.clone()],
            FieldMacros::Relationship(relationship) => vec![relationship.delete_statement.clone()],
            _ => vec![],
        }
    }

    pub fn delete_many_statements(&self) -> Vec<TokenStream> {
        match self {
            FieldMacros::Pk(pk) => vec![pk.delete_many_statement.clone()],
            FieldMacros::Plain(column) => vec![column.delete_many_statement.clone()],
            FieldMacros::Relationship(relationship) => vec![relationship.delete_many_statement.clone()],
            _ => vec![],
        }
    }
}
