mod store;
mod delete;
mod query;

use crate::field::FieldMacros;
use crate::table::TableDef;
use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use syn::Type;
use crate::rest::{to_http_endpoints, FunctionDef};
use crate::macro_utils;

pub struct EntityMacros {
    pub entity_name: Ident,
    pub entity_type: Type,
    pub pk_name: Ident,
    pub pk_type: Type,
    pub field_names: Vec<Ident>,
    pub table_definitions: Vec<TableDef>,
    pub struct_inits: Vec<TokenStream>,
    pub struct_inits_with_query: Vec<TokenStream>,
    pub struct_default_inits: Vec<TokenStream>,
    pub range_queries: Vec<TokenStream>,
    pub stream_queries: Vec<(TokenStream,TokenStream)>,
    pub store_statements: Vec<TokenStream>,
    pub store_many_statements: Vec<TokenStream>,
    pub delete_statements: Vec<TokenStream>,
    pub delete_many_statements: Vec<TokenStream>,
    pub function_defs: Vec<FunctionDef>,
}

impl EntityMacros {
    pub fn new(entity_name: Ident, entity_type: Type, pk_name: Ident, pk_type: Type, field_macros: Vec<FieldMacros>) -> Result<EntityMacros, syn::Error> {
        let mut field_names = Vec::new();
        let mut table_definitions = Vec::new();
        let mut range_queries = Vec::new();
        let mut stream_queries = Vec::new();
        let mut struct_inits = Vec::new();
        let mut struct_inits_with_query = Vec::new();
        let mut struct_default_inits = Vec::new();
        let mut store_statements = Vec::new();
        let mut delete_statements = Vec::new();
        let mut store_many_statements = Vec::new();
        let mut delete_many_statements = Vec::new();
        let mut function_defs = Vec::new();

        for column in field_macros.iter() {
            field_names.push(column.field_name().clone());
            table_definitions.extend(column.table_definitions());
            range_queries.extend(column.range_queries());
            stream_queries.extend(column.stream_queries());
            struct_inits.push(column.struct_init());
            struct_inits_with_query.push(column.struct_init_with_query());
            struct_default_inits.push(column.struct_default_init());
            store_statements.extend(column.store_statements());
            delete_statements.extend(column.delete_statements());
            store_many_statements.extend(column.store_many_statements());
            delete_many_statements.extend(column.delete_many_statements());
            function_defs.extend(column.function_defs())
        }

        Ok(EntityMacros {
            entity_name,
            entity_type,
            pk_name,
            pk_type,
            field_names,
            table_definitions,
            struct_inits,
            struct_inits_with_query,
            struct_default_inits,
            range_queries,
            stream_queries,
            store_statements,
            store_many_statements,
            delete_statements,
            delete_many_statements,
            function_defs,
        })
    }

    pub fn expand(&self) -> TokenStream {
        let entity_name = &self.entity_name;
        let entity_type = &self.entity_type;
        let pk_name = &self.pk_name;
        let pk_type = &self.pk_type;

        let struct_default_inits = &self.struct_default_inits;

        let store_statements = &self.store_statements;
        let store_many_statements = &self.store_many_statements;
        let delete_statements = &self.delete_statements;
        let delete_many_statements = &self.delete_many_statements;

        let mut function_defs = Vec::new();
        function_defs.push(store::fn_def(&entity_name, &entity_type, &pk_name, &pk_type, &store_statements));
        function_defs.extend(self.function_defs.clone());
        function_defs.push(delete::fn_def(&entity_name, &entity_type, &pk_name, &pk_type, &delete_statements));

        let struct_inits = &self.struct_inits;
        let queries = &self.range_queries;

        let function_streams: Vec<TokenStream> = function_defs.iter().map(|f| f.fn_stream.clone()).collect::<Vec<_>>();
        let table_definition_streams: Vec<TokenStream> = self.table_definitions.iter().map(|table_def| table_def.definition.clone()).collect();
        let tests = function_defs.iter().filter_map(|f| f.test_stream.clone()).collect::<Vec<_>>();
        let (endpoint_handlers, route_chains, route_tests) = to_http_endpoints(&function_defs);

        let entity_tests = format_ident!("{}_tests", entity_name.to_string().to_lowercase());
        let entity_literal = Literal::string(&entity_name.to_string());

        let (stream_query_ident, stream_query_struct) = query::stream_query_struct_macro(entity_name, &self.stream_queries);
        let struct_inits_with_query = &self.struct_inits_with_query;
        let field_names = &self.field_names;
        
        let expanded = quote! {
            #stream_query_struct
            // table definitions are not in the impl object because they are accessed globally with semantic meaning
            #(#table_definition_streams)*
            // utoipa and axum query structs to map query and path params into
            #(#queries)*
            // axum endpoints cannot be in the impl object https://docs.rs/axum/latest/axum/attr.debug_handler.html#limitations
            #(#endpoint_handlers)*
    
            impl #entity_name {
                #(#function_streams)*
                
                pub fn sample() -> Self {
                    #entity_name::sample_with(&#pk_type::default(), 0)
                }
    
                pub fn sample_with(pk: &#pk_type, sample_index: usize) -> Self {
                    #entity_name {
                        #(#struct_default_inits),*
                    }
                }
    
                pub fn sample_many(n: usize) -> Vec<#entity_type> {
                    let mut sample_index = 0;
                    std::iter::successors(Some((#pk_type::default(), None)), |(prev_pointer, _)| {
                        let new_entity = #entity_type::sample_with(prev_pointer, sample_index);
                        sample_index += 1;
                        Some((prev_pointer.next(), Some(new_entity)))
                    })
                    .filter_map(|(_, instance)| instance)
                    .take(n)
                    .collect()
                }
    
                fn compose(tx: &ReadTransaction, pk: &#pk_type) -> Result<#entity_type, AppError> {
                    Ok(#entity_name {
                        #(#struct_inits),*
                    })
                }
    
                fn compose_with_filter(tx: &ReadTransaction, pk: &#pk_type, streaming_query: #stream_query_ident) -> Result<Option<#entity_type>, AppError> {
                    // First: fetch & filter every column, short‑circuit on mismatch
                    #(#struct_inits_with_query)*
                    Ok(Some(#entity_type {
                        #(#field_names,)*
                    }))
                }
                
                pub fn store(tx: &WriteTransaction, instance: &#entity_type) -> Result<(), AppError> {
                    #(#store_statements)*
                    Ok(())
                }
    
                pub fn store_many(tx: &WriteTransaction, instances: &Vec<#entity_type>) -> Result<(), AppError> {
                    #(#store_many_statements)*
                    Ok(())
                }
    
                pub fn delete(tx: &WriteTransaction, pk: &#pk_type) -> Result<(), AppError> {
                    #(#delete_statements)*
                    Ok(())
                }
    
                pub fn delete_many(tx: &WriteTransaction, pks: &Vec<#pk_type>) -> Result<(), AppError> {
                    #(#delete_many_statements)*
                    Ok(())
                }
    
                pub fn routes() -> OpenApiRouter<RequestState> {
                    OpenApiRouter::new()
                        #(#route_chains)*
                }
            }
    
            #[cfg(test)]
            mod #entity_tests {
                use super::*;
    
                fn init_temp_db(name: &str) -> Arc<Database> {
                    let dir = std::env::temp_dir().join("redbit").join(name).join(#entity_literal);
                    if !dir.exists() {
                        std::fs::create_dir_all(dir.clone()).unwrap();
                    }
                    let db_path = dir.join(format!("{}_{}.redb", #entity_literal, rand::random::<u64>()));
                    Arc::new(Database::create(db_path).expect("Failed to create database"))
                }
    
                #[tokio::test]
                async fn test_entity_api() {
                    let db = init_temp_db("api");
                    let entity_count: usize = 3;
                    #(#tests)*
                }
    
                #[tokio::test]
                async fn test_entity_rest_api() {
                    let db = init_temp_db("rest-api");
                    let router = build_router(RequestState { db: Arc::clone(&db) }, None).await;
                    let server = axum_test::TestServer::new(router).unwrap();
                    #(#route_tests)*
                }
            }
        };
        // eprintln!("----------------------------------------------------------");
        macro_utils::write_stream_and_return(expanded, &entity_name)
    }

}
