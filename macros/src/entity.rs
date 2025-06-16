use crate::column::DbColumnMacros;
use crate::field_parser::*;
use crate::http::ParamExtraction::{FromBody, FromPath};
use crate::http::{EndpointDef, FunctionDef, GetParam, HttpMethod, PostParam};
use crate::pk::DbPkMacros;
use crate::relationship::{DbRelationshipMacros, TransientMacros};
use crate::table::TableDef;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
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
    pub fn new(entity_name: &Ident, entity_type: &Type, field_defs: FieldDefs) -> Result<EntityMacros, syn::Error> {
        let FieldDefs { pk, columns, relationships, transients } = field_defs;
        let column_macros = columns
            .into_iter()
            .map(|entity_column| DbColumnMacros::new(entity_column, entity_name, entity_type, &pk))
            .collect::<Result<Vec<DbColumnMacros>, syn::Error>>()?;
        let relationship_macros = relationships.into_iter().map(|rel| DbRelationshipMacros::new(rel, entity_name, &pk)).collect();
        Ok(EntityMacros {
            entity_name: entity_name.clone(),
            entity_type: entity_type.clone(),
            pk: DbPkMacros::new(entity_name, entity_type, &pk),
            columns: column_macros,
            relationships: relationship_macros,
            transients: TransientMacros::new(transients),
        })
    }

    pub fn table_definitions(&self) -> Vec<TableDef> {
        let mut table_definitions = Vec::new();
        table_definitions.push(self.pk.table_def.clone());
        for column in &self.columns {
            table_definitions.extend(column.table_definitions.clone());
        }
        table_definitions
    }

    pub fn struct_inits(&self) -> Vec<TokenStream> {
        let mut struct_inits = Vec::new();
        for column in &self.columns {
            struct_inits.push(column.struct_init.clone());
        }
        for relationship in &self.relationships {
            struct_inits.push(relationship.struct_init.clone());
        }
        for transient in &self.transients {
            struct_inits.push(transient.struct_default_init.clone());
        }
        struct_inits
    }

    pub fn struct_default_inits(&self) -> Vec<TokenStream> {
        let mut struct_default_inits = Vec::new();
        for column in &self.columns {
            struct_default_inits.push(column.struct_default_init.clone());
        }
        for relationship in &self.relationships {
            struct_default_inits.push(relationship.struct_default_init.clone());
        }
        for transient in &self.transients {
            struct_default_inits.push(transient.struct_default_init.clone());
        }
        struct_default_inits
    }

    pub fn function_defs(&self) -> Vec<FunctionDef> {
        let mut function_defs = vec![];
        function_defs.extend(self.pk.function_defs.clone());
        function_defs.push(self.store_fn_def());
        function_defs.push(self.delete_fn_def());
        for column in &self.columns {
            function_defs.extend(column.function_defs.clone());
        }
        for relationship in &self.relationships {
            function_defs.push(relationship.function_def.clone());
        }
        function_defs
    }

    pub fn store_fn_def(&self) -> FunctionDef {
        let pk_name = &self.pk.definition.field.name;
        let pk_type = &self.pk.definition.field.tpe;
        let entity_type = &self.entity_type;
        let entity_name = &self.entity_name;
        let fn_name = format_ident!("store_and_commit");
        let store_statements = self.store_statements();
        let fn_stream = quote! {
            pub fn #fn_name(db: &::redb::Database, instance: &#entity_type) -> Result<#pk_type, AppError> {
               let tx = db.begin_write()?;
               {
                   #(#store_statements)*
               }
               tx.commit()?;
               Ok(instance.#pk_name.clone())
           }
        };
        FunctionDef {
            entity_name: entity_name.clone(),
            fn_name: fn_name.clone(),
            return_type: syn::parse_quote!(#pk_type),
            fn_stream,
            endpoint_def: Some(EndpointDef {
                param_extraction: FromBody(PostParam {
                    name: format_ident!("body"),
                    ty: entity_type.clone(),
                    content_type: "application/json".to_string(),
                }),
                method: HttpMethod::POST,
                endpoint: format!("/{}", entity_name.to_string().to_lowercase()),
                fn_call: quote! { #entity_name::#fn_name(&db, &body) },
            }),
        }
    }

    pub fn store_statements(&self) -> Vec<TokenStream> {
        let mut statements = vec![self.pk.store_statement.clone()];
        for column in &self.columns {
            statements.push(column.store_statement.clone());
        }
        for relationship in &self.relationships {
            statements.push(relationship.store_statement.clone());
        }
        statements
    }

    pub fn delete_fn_def(&self) -> FunctionDef {
        let pk_type = &self.pk.definition.field.tpe;
        let pk_name = &self.pk.definition.field.name;
        let entity_name = &self.entity_name;
        let fn_name = format_ident!("delete_and_commit");
        let delete_statements = self.delete_statements();
        let fn_stream = quote! {
            pub fn #fn_name(db: &::redb::Database, pk: &#pk_type) -> Result<(), AppError> {
               let tx = db.begin_write()?;
               {
                   #(#delete_statements)*
               }
               tx.commit()?;
               Ok(())
           }
        };
        FunctionDef {
            entity_name: entity_name.clone(),
            fn_name: fn_name.clone(),
            return_type: syn::parse_quote!(()),
            fn_stream,
            endpoint_def: Some(EndpointDef {
                param_extraction: FromPath(vec![GetParam { name: pk_name.clone(), ty: pk_type.clone(), description: "Primary key".to_string() }]),
                method: HttpMethod::DELETE,
                endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), pk_name, pk_name),
                fn_call: quote! { #entity_name::#fn_name(&db, &#pk_name) },
            }),
        }
    }

    pub fn store_many_statements(&self) -> Vec<TokenStream> {
        let mut statements = vec![self.pk.store_many_statement.clone()];
        for column in &self.columns {
            statements.push(column.store_many_statement.clone());
        }
        for relationship in &self.relationships {
            statements.push(relationship.store_many_statement.clone());
        }
        statements
    }

    pub fn delete_statements(&self) -> Vec<TokenStream> {
        let mut statements = vec![self.pk.delete_statement.clone()];
        for column in &self.columns {
            statements.push(column.delete_statement.clone());
        }
        for relationship in &self.relationships {
            statements.push(relationship.delete_statement.clone());
        }
        statements
    }

    pub fn delete_many_statements(&self) -> Vec<TokenStream> {
        let mut statements = vec![self.pk.delete_many_statement.clone()];
        for column in &self.columns {
            statements.push(column.delete_many_statement.clone());
        }
        for relationship in &self.relationships {
            statements.push(relationship.delete_many_statement.clone());
        }
        statements
    }
}
