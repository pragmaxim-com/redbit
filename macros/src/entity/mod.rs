use crate::field::FieldMacros;
use crate::field_parser::{FieldDef, KeyDef};
use crate::rest::Rest;
use crate::storage;
use crate::table::{DictTableDefs, IndexTableDefs, PlainTableDef};
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse_quote, ItemStruct, Type};
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
mod manual_accessors;

fn vec_inner(ty: &Type) -> Option<Type> {
    if let Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            if seg.ident == "Vec" {
                if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(t)) = ab.args.first() {
                        return Some(t.clone());
                    }
                }
            }
        }
    }
    None
}

pub fn new(item_struct: &ItemStruct) -> Result<(KeyDef, Vec<FieldDef>, TokenStream), syn::Error> {
    let entity_name = &item_struct.ident;
    let (entity_def, one_to_many_parent_def, field_macros, col_defs) =
        FieldMacros::new(item_struct, entity_name, parse_quote! { #entity_name })?;
    // Collect write_from source field names to tailor sampling of transient fields.
    let write_from_sources: Vec<String> = col_defs
        .iter()
        .filter_map(|c| {
            if let crate::field_parser::ColumnDef::Relationship(_, Some(wf), _, _) = c {
                Some(wf.from.to_string())
            } else {
                None
            }
        })
        .collect();
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
        // If this field is a write_from source, sample it relative to the previous pk so hashes exist.
        let fname = field_macro.field_def().name.clone();
        let fname_str = fname.to_string();
        if write_from_sources.contains(&fname_str) {
            // For write_from sources (transient fields feeding hooks), sample from previous pk so referenced hashes exist.
            let init = if let Some(inner) = vec_inner(&field_macro.field_def().tpe) {
                quote! {
                    let #fname = <#inner as Sampleable>::sample_many_from(3, pk.total_index().saturating_sub(1) as usize);
                }
            } else {
                quote! { let #fname = Default::default(); }
            };
            struct_default_inits.push(init.clone());
            struct_default_inits_with_query.push(init);
            store_statements.extend(field_macro.store_statements());
            delete_statements.extend(field_macro.delete_statements());
            delete_many_statements.extend(field_macro.delete_many_statements());
            column_function_defs.extend(field_macro.function_defs());
            continue;
        }
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
        context::definition(&entity_def),
        context::begin_write_fn_def(&entity_def),
        context::new_write_fn_def(&entity_def),
        context::begin_read_fn_def(&entity_def),
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

    let table_info_struct = info::table_info_struct(&entity_def, &table_info_items);
    let filter_query_struct = query::filter_query(&entity_def.query_type, &filter_queries);
    let tx_context_structs = context::tx_context(&entity_def, &tx_context_items);
    let range_query_structs = range_queries.into_iter().map(|rq| rq.stream).collect::<Vec<_>>();

    let api_functions: Vec<TokenStream> = function_defs.iter().map(|f| f.fn_stream.clone()).collect::<Vec<_>>();

    let db_defs = storage::get_db_defs(&plain_table_defs, &dict_table_defs, &index_table_defs);

    let Rest { endpoint_handlers, routes: api_routes } =
        Rest::new(&function_defs);

    let test_suite = tests::test_suite(&entity_def, one_to_many_parent_def.clone(), &function_defs);
    let manual_tokens = manual_accessors::emit(&entity_def, &col_defs);

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
            #manual_tokens
            // unit tests and rest api tests
            #test_suite
        };
    Ok((key_def.clone(), field_defs, stream))
}
