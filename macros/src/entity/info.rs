use crate::endpoint::EndpointDef;
use crate::rest::{FunctionDef, HttpMethod};
use crate::table::TableDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};

pub fn table_info_fn(entity_name: &Ident, table_defs: &Vec<TableDef>) -> FunctionDef {
    let stats_getters = table_defs.iter().map(|td| {
        let table_name = td.name.to_string();
        let table_ident = &td.name;
        let multimap_flag = td.multimap;

        let open_method = if multimap_flag {
            quote!(open_multimap_table)
        } else {
            quote!(open_table)
        };

        quote! {
            {
                use redb::TableStats;
                let tx = db.begin_read()?;
                let table = tx.#open_method(#table_ident)?;
                let stats = table.stats()?;
                tables.push(TableInfo {
                    table_name: #table_name.to_string(),
                    multimap: #multimap_flag,
                    tree_height: stats.tree_height(),
                    leaf_pages: stats.leaf_pages(),
                    branch_pages: stats.branch_pages(),
                    stored_leaf_bytes: stats.stored_bytes(),
                    metadata_bytes: stats.metadata_bytes(),
                    fragmented_bytes: stats.fragmented_bytes(),
                });
            }
        }
    });
    let fn_name = format_ident!("table_info");
    let fn_stream = quote! {
        pub fn #fn_name(db: &Database) -> Result<Vec<TableInfo>, AppError> {
            let mut tables = Vec::new();
            #(#stats_getters)*
            Ok(tables)
        }
    };

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            entity_name: entity_name.clone(),
            fn_name: fn_name.clone(),
            params: vec![],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<Vec<TableInfo>>, AppError> {
                    #entity_name::#fn_name(&state.db).map(AppJson)
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/json", body = Vec<TableInfo>),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{}", entity_name.to_string().to_lowercase(), fn_name),
        }.to_endpoint()),
        test_stream: None,
        bench_stream: None
    }

}
