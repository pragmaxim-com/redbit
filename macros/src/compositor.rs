use proc_macro2::TokenStream;
use quote::quote;
use crate::entity_macros::EntityMacros;
use crate::http_macros::{to_http_endpoint, FunctionDef, HttpEndpointMacro};
use crate::macro_utils;

pub fn expand(entity_macros: EntityMacros) -> TokenStream {
    let struct_ident = &entity_macros.struct_name;
    let (pk_column, db_pk_macros) = &entity_macros.pk;
    let pk_table_name = db_pk_macros.table_name.clone();
    let pk_table_definition = db_pk_macros.table_definition.clone();
    let pk_store_statement = db_pk_macros.store_statement.clone();
    let pk_store_many_statement = db_pk_macros.store_many_statement.clone();
    let pk_delete_statement = db_pk_macros.delete_statement.clone();
    let pk_delete_many_statement = db_pk_macros.delete_many_statement.clone();

    let mut table_definitions = Vec::new();
    let mut store_statements = Vec::new();
    let mut store_many_statements = Vec::new();
    let mut struct_initializers = Vec::new();
    let mut delete_statements = Vec::new();
    let mut delete_many_statements = Vec::new();
    let mut function_defs: Vec<FunctionDef> = Vec::new();
    function_defs.extend(db_pk_macros.function_defs.clone());

    for (_, db_column_macros) in &entity_macros.columns {
        table_definitions.extend(db_column_macros.table_definitions.clone());
        struct_initializers.push(db_column_macros.struct_initializer.clone());
        store_statements.push(db_column_macros.store_statement.clone());
        store_many_statements.push(db_column_macros.store_many_statement.clone());
        function_defs.extend(db_column_macros.function_defs.clone());
        delete_statements.push(db_column_macros.delete_statement.clone());
        delete_many_statements.push(db_column_macros.delete_many_statement.clone());
    }

    for (_, db_relationship_macros) in &entity_macros.relationships {
        struct_initializers.push(db_relationship_macros.struct_initializer.clone());
        store_statements.push(db_relationship_macros.store_statement.clone());
        store_many_statements.push(db_relationship_macros.store_many_statement.clone());
        function_defs.push(db_relationship_macros.function_def.clone());
        delete_statements.push(db_relationship_macros.delete_statement.clone());
        delete_many_statements.push(db_relationship_macros.delete_many_statement.clone());
    }

    for (_, macros) in &entity_macros.transients {
        struct_initializers.push(macros.struct_initializer.clone());
    }
    let function_streams: Vec<TokenStream> = function_defs.iter().map(|f| f.stream.clone()).collect::<Vec<_>>();
    let table_definition_streams: Vec<TokenStream> = table_definitions.iter().map(|(_, stream)| stream.clone()).collect();
    let endpoints: Vec<HttpEndpointMacro> = function_defs.iter().filter_map(|fn_def| to_http_endpoint(fn_def)).collect();

    let mut table_lines = Vec::new();
    table_lines.push(format!("| PK_Table      |  {}", &pk_table_name));
    for (column_table_name, _) in table_definitions.iter() {
        table_lines.push(format!("| Index_Table   |  {}", column_table_name));
    }
    macro_utils::write_to_local_file(table_lines, "tables", &struct_ident);

    let mut entity_lines = Vec::new();
    for endpoint in &endpoints {
        let line = format!("| Endpoint      |  {}", endpoint.endpoint);
        eprintln!("{}", line);
        entity_lines.push(line);
    }
    macro_utils::write_to_local_file(entity_lines, "endpoints", &struct_ident);

    let endpoint_macros: Vec<TokenStream> = endpoints.iter().map(|e| e.handler.clone()).collect();
    let route_chains: Vec<TokenStream> =
        endpoints
            .into_iter()
            .map(|e| (e.endpoint, e.fn_name))
            .map(|(endpoint, function_name)| {
                quote! {
                        .route(#endpoint, ::axum::routing::get(#function_name))
                    }
            })
            .collect();

    let pk_ident = pk_column.field.name.clone();
    let pk_type = pk_column.field.tpe.clone();

    let expanded = quote! {
            // table definitions are not in the impl object because they are accessed globally with semantic meaning
            #pk_table_definition
            #(#table_definition_streams)*
            // axum endpoints cannot be in the impl object https://docs.rs/axum/latest/axum/attr.debug_handler.html#limitations
            #(#endpoint_macros)*

            impl #struct_ident {

                #(#function_streams)*

                fn compose(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#struct_ident, AppError> {
                    Ok(#struct_ident {
                        #pk_ident: pk.clone(),
                        #(#struct_initializers),*
                    })
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

                pub fn store_many(write_tx: &::redb::WriteTransaction, instances: &Vec<#struct_ident>) -> Result<(), AppError> {
                    #pk_store_many_statement
                    #(#store_many_statements)*
                    Ok(())
                }

                pub fn store(write_tx: &::redb::WriteTransaction, instance: &#struct_ident) -> Result<(), AppError> {
                    #pk_store_statement
                    #(#store_statements)*
                    Ok(())
                }
                pub fn store_and_commit(db: &::redb::Database, instance: &#struct_ident) -> Result<(), AppError> {
                    let write_tx = db.begin_write()?;
                    {
                        #pk_store_statement
                        #(#store_statements)*
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
    eprintln!("----------------------------------------------------------");
    macro_utils::write_stream_and_return(expanded, &struct_ident)
}
