use crate::entity::EntityMacros;
use crate::http::{to_http_endpoints, FunctionDef};
use crate::macro_utils;
use proc_macro2::TokenStream;
use quote::quote;

pub fn expand(entity_macros: EntityMacros) -> TokenStream {
    let entity_name = &entity_macros.entity_name;
    let entity_type = &entity_macros.entity_type;
    let db_pk_macros = &entity_macros.pk;

    let table_definitions = entity_macros.table_definitions();
    let struct_inits = entity_macros.struct_inits();
    let struct_default_inits = entity_macros.struct_default_inits();
    let function_defs: Vec<FunctionDef> = entity_macros.function_defs();
    
    let store_statements = entity_macros.store_statements();
    let store_many_statements = entity_macros.store_many_statements();
    let delete_statements = entity_macros.delete_statements();
    let delete_many_statements = entity_macros.delete_many_statements();

    let function_streams: Vec<TokenStream> = function_defs.iter().map(|f| f.fn_stream.clone()).collect::<Vec<_>>();
    let table_definition_streams: Vec<TokenStream> = table_definitions.iter().map(|table_def| table_def.definition.clone()).collect();

    let (endpoints, route_chains) = to_http_endpoints(function_defs);
    let endpoint_macros: Vec<TokenStream> = endpoints.iter().map(|e| e.handler.clone()).collect();

    let table_lines = table_definitions.iter().map(|table_def| format!("| Table         |  {}", table_def.name)).collect();
    macro_utils::write_to_local_file(table_lines, "tables", &entity_name);
    let entity_lines = endpoints.iter().map(|endpoint| format!("| Endpoint      |  {}", endpoint)).collect();
    macro_utils::write_to_local_file(entity_lines, "endpoints", &entity_name);

    let pk_name = db_pk_macros.definition.field.name.clone();
    let pk_type = db_pk_macros.definition.field.tpe.clone();

    let expanded = quote! {
        // table definitions are not in the impl object because they are accessed globally with semantic meaning
        #(#table_definition_streams)*

        // axum endpoints cannot be in the impl object https://docs.rs/axum/latest/axum/attr.debug_handler.html#limitations
        #(#endpoint_macros)*

        impl #entity_name {
            #(#function_streams)*

            pub fn sample_with(pk: &#pk_type) -> Self {
                #entity_name {
                    #pk_name: pk.clone(),
                    #(#struct_default_inits),*
                }
            }

            pub fn sample() -> Self {
                #entity_name::sample_with(&#pk_type::default())
            }

            fn compose(tx: &::redbit::redb::ReadTransaction, pk: &#pk_type) -> Result<#entity_type, AppError> {
                Ok(#entity_name {
                    #pk_name: pk.clone(),
                    #(#struct_inits),*
                })
            }

            pub fn store(tx: &::redbit::redb::WriteTransaction, instance: &#entity_type) -> Result<(), AppError> {
                #(#store_statements)*
                Ok(())
            }

            pub fn store_many(tx: &::redbit::redb::WriteTransaction, instances: &Vec<#entity_type>) -> Result<(), AppError> {
                #(#store_many_statements)*
                Ok(())
            }

            pub fn delete(tx: &::redbit::redb::WriteTransaction, pk: &#pk_type) -> Result<(), AppError> {
                #(#delete_statements)*
                Ok(())
            }

            pub fn delete_many(tx: &::redbit::redb::WriteTransaction, pks: &Vec<#pk_type>) -> Result<(), AppError> {
                #(#delete_many_statements)*
                Ok(())
            }

            pub fn routes() -> redbit::utoipa_axum::router::OpenApiRouter<RequestState> {
                redbit::utoipa_axum::router::OpenApiRouter::new()
                    #(#route_chains)*
            }
        }
    };
    // eprintln!("----------------------------------------------------------");
    macro_utils::write_stream_and_return(expanded, &entity_name)
}
