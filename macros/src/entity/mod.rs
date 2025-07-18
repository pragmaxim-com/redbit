use crate::field::FieldMacros;
use crate::field_parser::{KeyDef};
use crate::rest::Rest;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, ItemStruct, Type};

pub mod query;
mod store;
mod delete;
mod sample;
mod compose;
mod tests;

pub fn new(item_struct: &ItemStruct) -> Result<(KeyDef, TokenStream), syn::Error> {
    let entity_ident = &item_struct.ident;
    let entity_type: Type = parse_quote! { #entity_ident };
    let stream_query_type = query::stream_query_type(&entity_type);
    let (key_def, parent_def, field_macros) =
        FieldMacros::new(&item_struct, entity_ident, &entity_type, &stream_query_type)?;
    let key = key_def.field_def();
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
    let mut column_function_defs = Vec::new();

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
        column_function_defs.extend(column.function_defs())
    }

    let mut function_defs = Vec::new();
    function_defs.push(store::store_and_commit_def(entity_ident, &entity_type, &key.name, &key.tpe, &store_statements));
    function_defs.push(store::store_def(entity_ident, &entity_type, &store_statements));
    function_defs.push(store::store_many_def(entity_ident, &entity_type, &store_many_statements));
    function_defs.extend(column_function_defs.clone());
    function_defs.push(delete::delete_and_commit_def(entity_ident, &entity_type, &key.name, &key.tpe, &delete_statements));
    function_defs.push(delete::delete_def(&key.tpe, &delete_statements));
    function_defs.push(delete::delete_many_def(&key.tpe, &delete_many_statements));

    let stream_query_struct = query::stream_query(&stream_query_type, &stream_queries);
    let range_query_structs = range_queries.into_iter().map(|rq| rq.stream).collect::<Vec<_>>();

    let api_functions: Vec<TokenStream> = function_defs.iter().map(|f| f.fn_stream.clone()).collect::<Vec<_>>();
    let sample_functions = sample::sample_token_streams(entity_ident, &entity_type, &key.tpe, &struct_default_inits);
    let compose_function = compose::compose_token_stream(entity_ident, &entity_type, &key.tpe, &struct_inits);
    let compose_with_filter_function = compose::compose_with_filter_token_stream(&entity_type, &key.tpe, &stream_query_type, &field_names, &struct_inits_with_query);
    let compose_functions = vec![compose_function, compose_with_filter_function];
    let table_definitions: Vec<TokenStream> = table_definitions.iter().map(|table_def| table_def.definition.clone()).collect();

    let parent_ident = parent_def.map(|p|p.parent_ident);
    let test_suite = tests::test_suite(entity_ident, parent_ident, &function_defs);

    let Rest { endpoint_handlers, routes, client_calls} = Rest::new(&function_defs);

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
    Ok((key_def, stream))
}
