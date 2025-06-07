use crate::db_column_macros::{DbColumnMacros};
use crate::db_pk_macros::DbPkMacros;
use crate::db_relationship_macros::{DbRelationshipMacros, TransientMacros};
use crate::field_parser::*;
use proc_macro2::Ident;

pub struct EntityMacros {
    pub struct_name: Ident,
    pub pk: (Pk, DbPkMacros),
    pub columns: Vec<(Column, DbColumnMacros)>,
    pub relationships: Vec<(Relationship, DbRelationshipMacros)>,
    pub transients: Vec<(Transient, TransientMacros)>,
}

impl EntityMacros {
    pub fn new(entity_ident: Ident, field_defs: FieldDefs) -> Result<EntityMacros, syn::Error> {
        let FieldDefs { pk, columns, relationships, transients } = field_defs;
        let pk_name = &pk.field.name;
        let pk_type = &pk.field.tpe;
        let mut column_macros: Vec<(Column, DbColumnMacros)> = Vec::new();
        for entity_column in columns.into_iter() {
            let column_name = &entity_column.field.name.clone();
            let column_type = &entity_column.field.tpe.clone();
            match entity_column.indexing {
                Indexing::Off => {
                    column_macros.push((
                        entity_column.clone(),
                        DbColumnMacros::plain(&entity_ident, pk_name, pk_type, column_name, column_type),
                    ));
                }
                Indexing::On { dictionary: false, range } => {
                    column_macros.push((
                        entity_column.clone(),
                        DbColumnMacros::indexed(&entity_ident, pk_name, pk_type, column_name, column_type, range),
                    ));
                }
                Indexing::On { dictionary: true, range: false } => {
                    column_macros.push((
                        entity_column.clone(),
                        DbColumnMacros::indexed_with_dict(&entity_ident, pk_name, pk_type, column_name, column_type),
                    ));
                }
                Indexing::On { dictionary: true, range: true } => {
                    return Err(syn::Error::new(column_name.span(), "Range indexing on dictionary columns is not supported"))
                }
            }
        }

        let db_pk_macro = DbPkMacros::new(&entity_ident, &pk);
        let mut relationship_macros: Vec<(Relationship, DbRelationshipMacros)> = Vec::new();
        for rel in relationships.iter() {
            let db_relationship_macros = DbRelationshipMacros::new(&entity_ident, &pk, rel.clone());
            relationship_macros.push((rel.clone(), db_relationship_macros));
        }
        let transient_macros = TransientMacros::new(transients);
        Ok(EntityMacros {
            struct_name: entity_ident,
            pk: (pk, db_pk_macro),
            columns: column_macros,
            relationships: relationship_macros,
            transients: transient_macros
        })
    }
}
