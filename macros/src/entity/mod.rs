use crate::field::FieldMacros;
use crate::field_parser::{FieldDef, KeyDef};
use crate::rest::Rest;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse_quote, ItemStruct, Type};
pub mod query;
mod store;
mod delete;
mod sample;
mod compose;
mod tests;
mod info;
pub mod init;
pub mod chain;

pub fn new(item_struct: &ItemStruct) -> Result<(KeyDef, Vec<FieldDef>, TokenStream), syn::Error> {
    let entity_ident = &item_struct.ident;
    let entity_type: Type = parse_quote! { #entity_ident };
    let stream_query_type = query::stream_query_type(&entity_type);
    let (key_def, one_to_many_parent_def, field_macros) =
        FieldMacros::new(item_struct, entity_ident, &entity_type, &stream_query_type)?;
    let key = key_def.field_def();
    let mut field_defs = Vec::new();
    let mut table_defs = Vec::new();
    let mut range_queries = Vec::new();
    let mut stream_queries = Vec::new();
    let mut struct_inits = Vec::new();
    let mut struct_inits_with_query = Vec::new();
    let mut struct_default_inits = Vec::new();
    let mut struct_default_inits_with_query = Vec::new();
    let mut store_statements = Vec::new();
    let mut delete_statements = Vec::new();
    let mut store_many_statements = Vec::new();
    let mut delete_many_statements = Vec::new();
    let mut column_function_defs = Vec::new();

    for field_macro in field_macros.iter() {
        field_defs.push(field_macro.field_def().clone());
        table_defs.extend(field_macro.table_definitions());
        range_queries.extend(field_macro.range_queries());
        stream_queries.extend(field_macro.stream_queries());
        struct_inits.push(field_macro.struct_init());
        struct_inits_with_query.push(field_macro.struct_init_with_query());
        struct_default_inits.push(field_macro.struct_default_init());
        struct_default_inits_with_query.push(field_macro.struct_default_init_with_query());
        store_statements.extend(field_macro.store_statements());
        delete_statements.extend(field_macro.delete_statements());
        store_many_statements.extend(field_macro.store_many_statements());
        delete_many_statements.extend(field_macro.delete_many_statements());
        column_function_defs.extend(field_macro.function_defs())
    }

    let field_names: Vec<Ident> = field_defs.iter().map(|f| f.name.clone()).collect();

    let mut function_defs = Vec::new();
    function_defs.push(store::store_and_commit_def(entity_ident, &entity_type, &key.name, &key.tpe, &store_statements));
    function_defs.push(store::store_def(entity_ident, &entity_type, &store_statements));
    function_defs.push(store::store_many_def(entity_ident, &entity_type, &store_many_statements));
    function_defs.extend(column_function_defs.clone());
    function_defs.push(delete::delete_and_commit_def(entity_ident, &entity_type, &key.name, &key.tpe, &delete_statements));
    function_defs.push(delete::delete_def(&key.tpe, &delete_statements));
    function_defs.push(delete::delete_many_def(&key.tpe, &delete_many_statements));
    function_defs.push(info::table_info_fn(entity_ident, &table_defs));
    function_defs.push(compose::compose_token_stream(entity_ident, &entity_type, &key.tpe, &struct_inits));
    function_defs.push(compose::compose_with_filter_token_stream(&entity_type, &key.tpe, &stream_query_type, &field_names, &struct_inits_with_query));
    function_defs.extend(sample::sample_token_fns(entity_ident, &entity_type, &key.tpe, &stream_query_type, &struct_default_inits, &struct_default_inits_with_query, &field_names));
    function_defs.extend(init::init(entity_ident, &key_def));

    let stream_query_struct = query::stream_query(&stream_query_type, &stream_queries);
    let range_query_structs = range_queries.into_iter().map(|rq| rq.stream).collect::<Vec<_>>();

    let table_definitions: Vec<TokenStream> = table_defs.iter().map(|table_def| table_def.definition.clone()).collect();
    let table_cache_definitions: Vec<TokenStream> = table_defs.iter().flat_map(|table_def| table_def.cache.clone().map(|c| c.1)).collect();

    let api_functions: Vec<TokenStream> = function_defs.iter().map(|f| f.fn_stream.clone()).collect::<Vec<_>>();

    let Rest { endpoint_handlers, routes: api_routes } = Rest::new(&function_defs);

    let test_suite = tests::test_suite(entity_ident, one_to_many_parent_def.clone(), &function_defs);

    let stream: TokenStream =
        quote! {
            // StreamQuery is passed from the rest api as POST body and used to filter the stream of entities
            #stream_query_struct
            // Query structs to map query params into
            #(#range_query_structs)*
            // table definitions are not in the impl object because they are accessed globally with semantic meaning
            #(#table_definitions)*
            // dictionary tables have cache
            #(#table_cache_definitions)*
            // axum endpoints cannot be in the impl object https://docs.rs/axum/latest/axum/attr.debug_handler.html#limitations
            #(#endpoint_handlers)*

            impl #entity_ident {
                // api functions are exposed to users
                #(#api_functions)*
                // axum routes
                #api_routes
            }
            // unit tests and rest api tests
            #test_suite
        };
    Ok((key_def, field_defs, stream))
}
