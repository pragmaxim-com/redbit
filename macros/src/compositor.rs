use crate::entity::EntityMacros;
use crate::http::{to_http_endpoints, FunctionDef};
use crate::macro_utils;
use proc_macro2::TokenStream;
use quote::quote;

pub fn expand(entity_macros: EntityMacros) -> TokenStream {
    let entity_name = &entity_macros.entity_name;
    let entity_type = &entity_macros.entity_type;
    let db_pk_macros = &entity_macros.pk;
    let pk_store_statement = db_pk_macros.store_statement.clone();
    let pk_store_many_statement = db_pk_macros.store_many_statement.clone();
    let pk_delete_statement = db_pk_macros.delete_statement.clone();
    let pk_delete_many_statement = db_pk_macros.delete_many_statement.clone();

    let mut table_definitions = Vec::new();
    let mut struct_inits = Vec::new();
    let mut struct_default_inits = Vec::new();
    let mut store_statements = Vec::new();
    let mut store_many_statements = Vec::new();
    let mut delete_statements = Vec::new();
    let mut delete_many_statements = Vec::new();
    let mut function_defs: Vec<FunctionDef> = Vec::new();
    function_defs.extend(db_pk_macros.function_defs.clone());
    table_definitions.push(db_pk_macros.table_def.clone());

    for db_column_macros in &entity_macros.columns {
        table_definitions.extend(db_column_macros.table_definitions.clone());
        struct_inits.push(db_column_macros.struct_init.clone());
        struct_default_inits.push(db_column_macros.struct_default_init.clone());
        store_statements.push(db_column_macros.store_statement.clone());
        store_many_statements.push(db_column_macros.store_many_statement.clone());
        delete_statements.push(db_column_macros.delete_statement.clone());
        delete_many_statements.push(db_column_macros.delete_many_statement.clone());
        function_defs.extend(db_column_macros.function_defs.clone());
    }

    for db_relationship_macros in &entity_macros.relationships {
        struct_inits.push(db_relationship_macros.struct_init.clone());
        struct_default_inits.push(db_relationship_macros.struct_default_init.clone());
        store_statements.push(db_relationship_macros.store_statement.clone());
        store_many_statements.push(db_relationship_macros.store_many_statement.clone());
        delete_statements.push(db_relationship_macros.delete_statement.clone());
        delete_many_statements.push(db_relationship_macros.delete_many_statement.clone());
        function_defs.push(db_relationship_macros.function_def.clone());
    }

    for transient_macros in &entity_macros.transients {
        struct_inits.push(transient_macros.struct_default_init.clone());
        struct_default_inits.push(transient_macros.struct_default_init.clone());
    }
    let function_streams: Vec<TokenStream> = function_defs.iter().map(|f| f.stream.clone()).collect::<Vec<_>>();
    let table_definition_streams: Vec<TokenStream> = table_definitions.iter().map(|table_def| table_def.definition.clone()).collect();

    let (endpoints, route_chains) = to_http_endpoints(function_defs);
    let endpoint_macros: Vec<TokenStream> = endpoints.iter().map(|e| e.handler.clone()).collect();

    let table_lines = table_definitions.iter().map(|table_def| format!("| Table         |  {}", table_def.name)).collect();
    macro_utils::write_to_local_file(table_lines, "tables", &entity_name);
    let entity_lines = endpoints.iter().map(|endpoint| format!("| Endpoint      |  {}", endpoint)).collect();
    macro_utils::write_to_local_file(entity_lines, "endpoints", &entity_name);

    let pk_ident = db_pk_macros.definition.field.name.clone();
    let pk_type = db_pk_macros.definition.field.tpe.clone();

    let expanded = quote! {
        // table definitions are not in the impl object because they are accessed globally with semantic meaning
        #(#table_definition_streams)*

        // axum endpoints cannot be in the impl object https://docs.rs/axum/latest/axum/attr.debug_handler.html#limitations
        #(#endpoint_macros)*

        impl #entity_name {
            #(#function_streams)*

            pub fn sample(pk: &#pk_type) -> Self {
                #entity_name {
                    #pk_ident: pk.clone(),
                    #(#struct_default_inits),*
                }
            }

            fn compose(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#entity_type, AppError> {
                Ok(#entity_name {
                    #pk_ident: pk.clone(),
                    #(#struct_inits),*
                })
            }

            pub fn store(write_tx: &::redb::WriteTransaction, instance: &#entity_type) -> Result<(), AppError> {
                #pk_store_statement
                #(#store_statements)*
                Ok(())
            }

            pub fn store_many(write_tx: &::redb::WriteTransaction, instances: &Vec<#entity_type>) -> Result<(), AppError> {
                #pk_store_many_statement
                #(#store_many_statements)*
                Ok(())
            }

            pub fn store_and_commit(db: &::redb::Database, instance: &#entity_type) -> Result<(), AppError> {
                let write_tx = db.begin_write()?;
                {
                    #pk_store_statement
                    #(#store_statements)*
                }
                write_tx.commit()?;
                Ok(())
            }

            pub fn delete(write_tx: &::redb::WriteTransaction, pk: &#pk_type) -> Result<(), AppError> {
                #pk_delete_statement
                #(#delete_statements)*
                Ok(())
            }

            pub fn delete_many(write_tx: &::redb::WriteTransaction, pks: &Vec<#pk_type>) -> Result<(), AppError> {
                #pk_delete_many_statement
                #(#delete_many_statements)*
                Ok(())
            }

            pub fn delete_and_commit(db: &::redb::Database, pk: &#pk_type) -> Result<(), AppError> {
                let write_tx = db.begin_write()?;
                {
                    #pk_delete_statement
                    #(#delete_statements)*
                }
                write_tx.commit()?;
                Ok(())
            }

            pub fn routes() -> axum::Router<RequestState> {
                axum::Router::new()
                    #(#route_chains)*
            }
        }
    };
    // eprintln!("----------------------------------------------------------");
    macro_utils::write_stream_and_return(expanded, &entity_name)
}
