use crate::field::FieldMacros;
use crate::rest::to_http_endpoints;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_quote, ItemStruct, Type};

mod store;
mod delete;
mod query;
mod sample;
mod compose;
mod tests;

pub struct EntityMacros {}

impl EntityMacros {
    pub fn new(item_struct: &ItemStruct) -> Result<TokenStream, syn::Error> {
        let entity_ident = &item_struct.ident;
        let entity_type: Type = parse_quote! { #entity_ident };
        let stream_query_suffix = format!("StreamQuery");
        let stream_query_ident = format_ident!("{}{}", entity_ident, &stream_query_suffix);
        let stream_query_type: Type = syn::parse_quote! { #stream_query_ident };

        let (pk, field_macros) =
            FieldMacros::new(&item_struct, entity_ident, &entity_type, &stream_query_type, &stream_query_suffix)?;

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
        function_defs.push(Self::store_and_commit_def(entity_ident, &entity_type, &pk.name, &pk.tpe, &store_statements));
        function_defs.push(Self::store_def(entity_ident, &entity_type, &store_statements));
        function_defs.push(Self::store_many_def(entity_ident, &entity_type, &store_many_statements));
        function_defs.extend(column_function_defs.clone());
        function_defs.push(Self::delete_and_commit_def(entity_ident, &entity_type, &pk.name, &pk.tpe, &delete_statements));
        function_defs.push(Self::delete_def(entity_ident, &pk.tpe, &delete_statements));
        function_defs.push(Self::delete_many_def(entity_ident, &pk.tpe, &delete_many_statements));

        let stream_query_struct = Self::query_struct_token_stream(&stream_query_type, &stream_queries);

        let sample_functions = Self::sample_token_streams(entity_ident, &entity_type, &pk.tpe, &struct_default_inits);
        let compose_function = Self::compose_token_stream(entity_ident, &entity_type, &pk.tpe, &struct_inits);
        let compose_with_filter_function = Self::compose_with_filter_token_stream(&entity_type, &pk.tpe, &stream_query_type, &field_names, &struct_inits_with_query);
        let compose_functions = vec![compose_function, compose_with_filter_function];
        let table_definitions: Vec<TokenStream> = table_definitions.iter().map(|table_def| table_def.definition.clone()).collect();

        let api_functions: Vec<TokenStream> = function_defs.iter().map(|f| f.fn_stream.clone()).collect::<Vec<_>>();
        let unit_tests = function_defs.iter().filter_map(|f| f.test_stream.clone()).collect::<Vec<_>>();
        let benches = function_defs.iter().filter_map(|f| f.bench_stream.clone()).collect::<Vec<_>>();

        let (endpoint_handlers, routes, route_tests, client_calls) = to_http_endpoints(&function_defs);
        let test_suite = tests::test_suite(entity_ident, unit_tests, route_tests, benches);

        let stream: TokenStream =
            quote! {
                // StreamQuery is passed from the rest api as POST body and used to filter the stream of entities
                #stream_query_struct
                // Query structs to map query params into
                #(#range_query_structs)*
                // table definitions are not in the impl object because they are accessed globally with semantic meaning
                #(#table_definitions)*
                // axum endpoints cannot be in the impl object https://docs.rs/axum/latest/axum/attr.debug_handler.html#limitations
                #(#endpoint_handlers)*

                impl #entity_ident {
                    // api functions are exposed to users
                    #(#api_functions)*
                    // sample functions are used to generate test data
                    #(#sample_functions)*
                    // compose functions build entities from db results
                    #(#compose_functions)*
                    // axum routes
                    #routes
                    // client calls are executed from node.js runtime
                    #client_calls
                }
                // unit tests and rest api tests
                #test_suite
            }.into();
        Ok(stream)
    }
}
