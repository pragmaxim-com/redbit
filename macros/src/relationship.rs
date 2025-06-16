mod get;
mod init;
mod store;
mod delete;

use crate::field_parser::*;
use crate::http::FunctionDef;
use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub struct TransientMacros {
    pub definition: TransientDef,
    pub struct_default_init: TokenStream,
}

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

impl TransientMacros {
    pub fn new(defs: Vec<TransientDef>) -> Vec<TransientMacros> {
        let mut transient_macros: Vec<TransientMacros> = Vec::new();
        for transient in defs {
            let field_name = &transient.field.name;
            let field_type = &transient.field.tpe;
            let struct_default_init = quote! {
                #field_name: #field_type::default()
            };
            transient_macros.push(TransientMacros { definition: transient, struct_default_init})
        }
        transient_macros
    }
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
                    struct_init: init::o2o_relation_init(child_name, child_type),
                    struct_default_init: init::o2o_relation_default_init(child_name, child_type),
                    store_statement: store::o2o_store_def(child_name, child_type),
                    store_many_statement: store::o2o_store_many_def(child_name, child_type),
                    delete_statement: delete::o2o_delete_def(child_type),
                    delete_many_statement: delete::o2o_delete_many_def(child_type),
                    function_def: get::o2o_def(entity_ident, child_name, child_type, &pk_name, &pk_type)
                }
            }
            Multiplicity::OneToMany => {
                DbRelationshipMacros {
                    definition: definition.clone(),
                    struct_init: init::o2m_relation_init(child_name, child_type),
                    struct_default_init: init::o2m_relation_default_init(child_name, child_type),
                    store_statement: store::o2m_store_def(child_name, child_type),
                    store_many_statement: store::o2m_store_many_def(child_name, child_type),
                    delete_statement: delete::o2m_delete_def(child_type),
                    delete_many_statement: delete::o2m_delete_many_def(child_type),
                    function_def: get::o2m_def(entity_ident, child_name, child_type, &pk_name, &pk_type)
                }
            }
        }
    }
}
