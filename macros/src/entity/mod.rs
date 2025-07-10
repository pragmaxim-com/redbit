use crate::field::FieldMacros;
use crate::rest::to_http_endpoints;
use proc_macro2::{Ident, TokenStream};
use syn::Type;

mod store;
mod delete;
mod query;
mod sample;
mod compose;
mod expand;
mod tests;

pub struct EntityMacros {
    pub entity_name: Ident,
    pub table_definitions: Vec<TokenStream>,
    pub sample_functions: Vec<TokenStream>,
    pub compose_functions: Vec<TokenStream>,
    pub range_query_structs: Vec<TokenStream>,
    pub stream_query_struct: TokenStream,
    pub api_functions: Vec<TokenStream>,
    pub endpoint_handlers: Vec<TokenStream>,
    pub routes: TokenStream,
    pub test_suite: TokenStream,
}

impl EntityMacros {
    pub fn new(entity_name: Ident, entity_type: Type, pk_name: Ident, pk_type: Type, stream_query_type: &Type, field_macros: Vec<FieldMacros>) -> Result<EntityMacros, syn::Error> {
        let mut field_names = Vec::new();
        let mut table_definitions = Vec::new();
        let mut range_query_structs = Vec::new();
        let mut stream_queries = Vec::new();
        let mut struct_inits = Vec::new();
        let mut struct_inits_with_query = Vec::new();
        let mut struct_default_inits = Vec::new();
        let mut store_statements = Vec::new();
        let mut delete_statements = Vec::new();
        let mut store_many_statements = Vec::new();
        let mut delete_many_statements = Vec::new();
        let mut column_function_defs = Vec::new();

        for column in field_macros.iter() {
            field_names.push(column.field_name().clone());
            table_definitions.extend(column.table_definitions());
            range_query_structs.extend(column.range_queries());
            stream_queries.extend(column.stream_queries());
            struct_inits.push(column.struct_init());
            struct_inits_with_query.push(column.struct_init_with_query());
            struct_default_inits.push(column.struct_default_init());
            store_statements.extend(column.store_statements());
            delete_statements.extend(column.delete_statements());
            store_many_statements.extend(column.store_many_statements());
            delete_many_statements.extend(column.delete_many_statements());
            column_function_defs.extend(column.function_defs())
        }

        let mut function_defs = Vec::new();
        function_defs.push(Self::store_and_commit_def(&entity_name, &entity_type, &pk_name, &pk_type, &store_statements));
        function_defs.push(Self::store_def(&entity_name, &entity_type, &store_statements));
        function_defs.push(Self::store_many_def(&entity_name, &entity_type, &store_many_statements));
        function_defs.extend(column_function_defs.clone());
        function_defs.push(Self::delete_and_commit_def(&entity_name, &entity_type, &pk_name, &pk_type, &delete_statements));
        function_defs.push(Self::delete_def(&entity_name, &pk_type, &delete_statements));
        function_defs.push(Self::delete_many_def(&entity_name, &pk_type, &delete_many_statements));

        let stream_query_struct = Self::query_struct_token_stream(&stream_query_type, &stream_queries);

        let sample_functions = Self::sample_token_streams(&entity_name, &entity_type, &pk_type, &struct_default_inits);
        let compose_function = Self::compose_token_stream(&entity_name, &entity_type, &pk_type, &struct_inits);
        let compose_with_filter_function = Self::compose_with_filter_token_stream(&entity_type, &pk_type, &stream_query_type, &field_names, &struct_inits_with_query);
        let compose_functions = vec![compose_function, compose_with_filter_function];
        let table_definitions = table_definitions.iter().map(|table_def| table_def.definition.clone()).collect();

        let api_functions: Vec<TokenStream> = function_defs.iter().map(|f| f.fn_stream.clone()).collect::<Vec<_>>();
        let unit_tests = function_defs.iter().filter_map(|f| f.test_stream.clone()).collect::<Vec<_>>();
        let benches = function_defs.iter().filter_map(|f| f.bench_stream.clone()).collect::<Vec<_>>();

        let (endpoint_handlers, routes, route_tests) = to_http_endpoints(&function_defs);
        let test_suite = tests::test_suite(&entity_name, unit_tests, route_tests, benches);

        Ok(EntityMacros {
            entity_name,
            stream_query_struct,
            table_definitions,
            compose_functions,
            sample_functions,
            range_query_structs,
            api_functions,
            endpoint_handlers,
            routes,
            test_suite
        })
    }
}
