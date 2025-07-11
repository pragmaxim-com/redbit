mod get;
mod init;
mod store;
mod delete;
mod query;

use crate::field_parser::*;
use crate::rest::FunctionDef;
use proc_macro2::{Ident, TokenStream};
use quote::format_ident;
use syn::Type;

pub struct DbRelationshipMacros {
    pub field_def: FieldDef,
    pub struct_init: TokenStream,
    pub stream_query_init: (TokenStream, TokenStream),
    pub struct_init_with_query: TokenStream,
    pub struct_default_init: TokenStream,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_def: FunctionDef,
}

impl DbRelationshipMacros {
    pub fn new(field_def: FieldDef, multiplicity: Multiplicity, entity_ident: &Ident, pk_name: &Ident, pk_type: &Type, stream_query_suffix: &str) -> DbRelationshipMacros {
        let child_name = &field_def.name; // e.g., "transactions"
        let child_type = &field_def.tpe; // e.g., the type `Transaction` from Vec<Transaction>
        let child_ident = match &child_type {
            Type::Path(p) => &p.path.segments.last().unwrap().ident,
            _ => panic!("Unsupported child type"),
        };
        let child_stream_query_ident = format_ident!("{}{}", child_ident, &stream_query_suffix);
        let child_stream_query_type: Type = syn::parse_quote! { #child_stream_query_ident };
        match multiplicity {
            Multiplicity::OneToOne => {
                DbRelationshipMacros {
                    field_def: field_def.clone(),
                    struct_init: init::one2one_relation_init(child_name, child_type),
                    stream_query_init: query::stream_query_init(child_name, &child_stream_query_type),
                    struct_init_with_query: init::one2one_relation_init_with_query(child_name, child_type),
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
                    field_def: field_def.clone(),
                    struct_init: init::one2opt_relation_init(child_name, child_type),
                    stream_query_init: query::stream_query_init(child_name, &child_stream_query_type),
                    struct_init_with_query: init::one2opt_relation_init_with_query(child_name, child_type),
                    struct_default_init: init::one2opt_relation_default_init(child_name, child_type),
                    store_statement: store::one2opt_store_def(child_name, child_type),
                    store_many_statement: store::one2opt_store_many_def(child_name, child_type),
                    delete_statement: delete::one2opt_delete_def(child_type),
                    delete_many_statement: delete::one2opt_delete_many_def(child_type),
                    function_def: get::one2opt_def(entity_ident, child_name, child_type, &pk_name, &pk_type)
                }
            }
            Multiplicity::OneToMany => {
                DbRelationshipMacros {
                    field_def: field_def.clone(),
                    struct_init: init::one2many_relation_init(child_name, child_type),
                    stream_query_init: query::stream_query_init(child_name, &child_stream_query_type),
                    struct_init_with_query: init::one2many_relation_init_with_query(child_name, child_type),
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
