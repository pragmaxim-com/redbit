use crate::column::{DbColumnMacros};
use crate::pk::DbPkMacros;
use crate::relationship::{DbRelationshipMacros, TransientMacros};
use crate::field_parser::*;
use proc_macro2::Ident;
use syn::Type;

pub struct EntityMacros {
    pub entity_name: Ident,
    pub entity_type: Type,
    pub pk: DbPkMacros,
    pub columns: Vec<DbColumnMacros>,
    pub relationships: Vec<DbRelationshipMacros>,
    pub transients: Vec<TransientMacros>,
}

impl EntityMacros {
    pub fn new(entity_ident: &Ident, entity_type: &Type, field_defs: FieldDefs) -> Result<EntityMacros, syn::Error> {
        let FieldDefs { pk, columns, relationships, transients } = field_defs;
        let column_macros =
            columns.into_iter()
                .map(|entity_column| DbColumnMacros::new(entity_column, entity_ident, entity_type, &pk.field.name, &pk.field.tpe))
                .collect::<Result<Vec<DbColumnMacros>, syn::Error>>()?;
        let relationship_macros =
            relationships.into_iter()
                .map(|rel| DbRelationshipMacros::new(rel, entity_ident, &pk))
                .collect();
        Ok(EntityMacros {
            entity_name: entity_ident.clone(),
            entity_type: entity_type.clone(),
            pk: DbPkMacros::new(entity_ident, entity_type, &pk),
            columns: column_macros,
            relationships: relationship_macros,
            transients: TransientMacros::new(transients)
        })
    }
}
