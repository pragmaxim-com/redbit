use crate::field::FieldMacros;
use crate::field_parser::{FieldDef, KeyDef};
use crate::rest::Rest;
use crate::storage;
use crate::storage::StorageDef;
use crate::table::{DictTableDefs, IndexTableDefs, PlainTableDef};
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse_quote, ItemStruct};
use crate::relationship::StoreStatement;

pub mod query;
mod store;
mod delete;
mod sample;
mod compose;
mod tests;
pub mod info;
pub mod init;
pub mod chain;
pub mod context;

pub fn new(item_struct: &ItemStruct) -> Result<(KeyDef, Vec<FieldDef>, TokenStream), syn::Error> {
    let entity_name = &item_struct.ident;
    let (entity_def, one_to_many_parent_def, field_macros) =
        FieldMacros::new(item_struct, entity_name, parse_quote! { #entity_name })?;
    let mut field_defs = Vec::new();
    let mut plain_table_defs: Vec<PlainTableDef> = Vec::new();
    let mut index_table_defs: Vec<IndexTableDefs> = Vec::new();
    let mut dict_table_defs: Vec<DictTableDefs> = Vec::new();
    let mut range_queries = Vec::new();
    let mut filter_queries = Vec::new();
    let mut table_info_items = Vec::new();
    let mut tx_context_items = Vec::new();
    let mut struct_inits = Vec::new();
    let mut struct_inits_with_query = Vec::new();
    let mut struct_default_inits = Vec::new();
    let mut struct_default_inits_with_query = Vec::new();
    let mut store_statements: Vec<StoreStatement> = Vec::new();
    let mut delete_statements = Vec::new();
    let mut delete_many_statements = Vec::new();
    let mut column_function_defs = Vec::new();

    for field_macro in field_macros.iter() {
        field_defs.push(field_macro.field_def().clone());
        plain_table_defs.extend(field_macro.plain_table_definitions());
        index_table_defs.extend(field_macro.index_table_definitions());
        dict_table_defs.extend(field_macro.dict_table_definitions());
        range_queries.extend(field_macro.range_queries());
        filter_queries.extend(field_macro.stream_queries());
        tx_context_items.extend(field_macro.tx_context_items());
        table_info_items.extend(field_macro.table_info_items());
        struct_inits.push(field_macro.struct_init());
        struct_inits_with_query.push(field_macro.struct_init_with_query());
        struct_default_inits.push(field_macro.struct_default_init());
        struct_default_inits_with_query.push(field_macro.struct_default_init_with_query());
        store_statements.extend(field_macro.store_statements());
        delete_statements.extend(field_macro.delete_statements());
        delete_many_statements.extend(field_macro.delete_many_statements());
        column_function_defs.extend(field_macro.function_defs())
    }

    let field_names: Vec<Ident> = field_defs.iter().map(|f| f.name.clone()).collect();
    let key_def = &entity_def.key_def.clone();

    let mut function_defs = vec![
        info::table_info_fn(&entity_def),
        store::persist_def(&entity_def, &store_statements),
        store::store_many_def(&entity_def, &store_statements),
        store::store_def(&entity_def, &store_statements),
        context::begin_write_fn_def(&entity_def.write_ctx_type),
        context::new_write_fn_def(&entity_def.write_ctx_type),
        context::begin_read_fn_def(&entity_def.read_ctx_type),
        delete::remove_def(&entity_def, &delete_statements),
        delete::delete_def(&entity_def, &delete_statements),
        delete::delete_many_def(&entity_def, &delete_many_statements),
        compose::compose_token_stream(&entity_def, &field_names, &struct_inits),
        compose::compose_with_filter_token_stream(&entity_def, &field_names, &struct_inits_with_query),
        compose::compose_many_token_stream(&entity_def),
        compose::compose_many_stream_token_stream(&entity_def),
    ];
    function_defs.extend(sample::sample_token_fns(&entity_def, &struct_default_inits, &struct_default_inits_with_query, &field_names));
    function_defs.extend(column_function_defs.clone());
    function_defs.extend(init::init(entity_name, key_def));

    let table_info_struct = info::table_info_struct(&entity_def.info_type, &table_info_items);
    let filter_query_struct = query::filter_query(&entity_def.query_type, &filter_queries);
    let tx_context_structs = context::tx_context(&entity_def.write_ctx_type, &entity_def.read_ctx_type, &tx_context_items);
    let range_query_structs = range_queries.into_iter().map(|rq| rq.stream).collect::<Vec<_>>();

    let api_functions: Vec<TokenStream> = function_defs.iter().map(|f| f.fn_stream.clone()).collect::<Vec<_>>();

    let StorageDef { db_defs, table_defs } = storage::get_db_defs(&plain_table_defs, &dict_table_defs, &index_table_defs);

    let Rest { endpoint_handlers, routes: api_routes } =
        Rest::new(&function_defs);

    let test_suite = tests::test_suite(&entity_def, one_to_many_parent_def.clone(), &function_defs);

    let stream: TokenStream =
        quote! {
            // Table info renders information about the tables used by the entity, including count of records !
            #table_info_struct
            // Query is passed from the rest api as POST body and used to filter the stream of entities
            #filter_query_struct
            // TxContext is used to open tables
            #tx_context_structs
            // Query structs to map query params into
            #(#range_query_structs)*
            // table definitions are not in the impl object because they are accessed globally with semantic meaning
            #(#table_defs)*
            // axum endpoints cannot be in the impl object https://docs.rs/axum/latest/axum/attr.debug_handler.html#limitations
            #(#endpoint_handlers)*

            impl #entity_name {
                // api functions are exposed to users
                #(#api_functions)*
                // axum routes
                #api_routes
                // entity fields have their own dbs
                #db_defs
            }
            // unit tests and rest api tests
            #test_suite
        };
    Ok((key_def.clone(), field_defs, stream))
}
