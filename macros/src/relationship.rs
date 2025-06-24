mod get;
mod init;
mod store;
mod delete;

use crate::field_parser::*;
use crate::rest::FunctionDef;
use proc_macro2::{Ident, TokenStream};

pub struct DbRelationshipMacros {
    pub definition: RelationshipDef,
    pub struct_init: TokenStream,
    pub struct_default_init: TokenStream,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_def: FunctionDef,
}

impl DbRelationshipMacros {
    pub fn new(definition: RelationshipDef, entity_ident: &Ident, pk_column: &PkDef) -> DbRelationshipMacros {
        let pk_type = pk_column.field.tpe.clone(); // BlockPointer
        let pk_name = pk_column.field.name.clone(); // BlockPointer
        let child_name = &definition.field.name; // e.g., "transactions"
        let child_type = &definition.field.tpe; // e.g., the type `Transaction` from Vec<Transaction>
        match definition.multiplicity {
            Multiplicity::OneToOne => {
                DbRelationshipMacros {
                    definition: definition.clone(),
                    struct_init: init::one2one_relation_init(child_name, child_type),
                    struct_default_init: init::one2one_relation_default_init(child_name, child_type),
                    store_statement: store::one2one_store_def(child_name, child_type),
                    store_many_statement: store::one2one_store_many_def(child_name, child_type),
                    delete_statement: delete::one2one_delete_def(child_type),
                    delete_many_statement: delete::one2one_delete_many_def(child_type),
                    function_def: get::one2one_def(entity_ident, child_name, child_type, &pk_name, &pk_type)
                }
            }
            Multiplicity::OneToOption => {
                DbRelationshipMacros {
                    definition: definition.clone(),
                    struct_init: init::one2option_relation_init(child_name, child_type),
                    struct_default_init: init::one2option_relation_default_init(child_name, child_type),
                    store_statement: store::one2option_store_def(child_name, child_type),
                    store_many_statement: store::one2option_store_many_def(child_name, child_type),
                    delete_statement: delete::one2option_delete_def(child_type),
                    delete_many_statement: delete::one2option_delete_many_def(child_type),
                    function_def: get::one2option_def(entity_ident, child_name, child_type, &pk_name, &pk_type)
                }
            }
            Multiplicity::OneToMany => {
                DbRelationshipMacros {
                    definition: definition.clone(),
                    struct_init: init::one2many_relation_init(child_name, child_type),
                    struct_default_init: init::one2many_relation_default_init(child_name, child_type),
                    store_statement: store::one2many_store_def(child_name, child_type),
                    store_many_statement: store::one2many_store_many_def(child_name, child_type),
                    delete_statement: delete::one2many_delete_def(child_type),
                    delete_many_statement: delete::one2many_delete_many_def(child_type),
                    function_def: get::one2many_def(entity_ident, child_name, child_type, &pk_name, &pk_type)
                }
            }
        }
    }
}
