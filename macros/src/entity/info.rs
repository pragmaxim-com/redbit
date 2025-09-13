use crate::endpoint::EndpointDef;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod};
use crate::table::{DictTableDefs, TableDef};
use crate::table::TableType;
use proc_macro2::{Ident, Literal};
use quote::{format_ident, quote};

pub fn table_info_fn(entity_name: &Ident, table_defs: &[TableDef], dict_table_defs: &[DictTableDefs]) -> FunctionDef {
    let plain_stats_getters = table_defs.iter().map(|td| {
        let table_var = td.var_name.to_string();
        let table_ident = &td.name;
        let table_type = format!("{:?}", &td.table_type);
        let open_method = match td.table_type {
            TableType::DictIndex | TableType::Index => quote!(open_multimap_table),
            _ => quote!(open_table),
        };

        quote! {
            {
                let tx = plain_db.begin_read()?;
                let table = tx.#open_method(#table_ident)?;
                let stats = table.stats()?;
                tables.push(stats_for_table(#table_var, #table_type, stats)?);
            }
        }
    });
    let dict_stats_getters = dict_table_defs.iter().map(|defs| {
        let db_name = defs.var_name.clone();
        let db_name_literal = Literal::string(&db_name.to_string());
        let dict_index_table_name = &defs.dict_index_table_def.name;
        let value_by_dict_pk_table_name = &defs.value_by_dict_pk_table_def.name;
        let value_to_dict_pk_table_name = &defs.value_to_dict_pk_table_def.name;
        let dict_pk_by_pk_table_name = &defs.dict_pk_by_pk_table_def.name;

        quote! {
            let db = index_dbs.get(#db_name_literal).cloned().unwrap();
            let dict_table = ReadOnlyDictTable::new(db, #dict_index_table_name, #value_by_dict_pk_table_name, #value_to_dict_pk_table_name, #dict_pk_by_pk_table_name)?;
            dict_table.stats()?.into_iter().for_each(|(table_type, stats)| {
                tables.push(stats_for_table(#db_name_literal, &table_type, stats).unwrap());
            });
        }
    });
    let fn_name = format_ident!("table_info");
    let fn_stream = quote! {
        pub fn #fn_name(storage: &Arc<Storage>) -> Result<Vec<TableInfo>, AppError> {
            let plain_db = &storage.plain_db;
            let index_dbs = &storage.index_dbs;
            fn stats_for_table(table_name: &str, table_type: &str, stats: TableStats) -> Result<TableInfo, AppError> {
                Ok(TableInfo {
                    table_name: table_name.to_string(),
                    table_type: table_type.to_string(),
                    tree_height: stats.tree_height(),
                    leaf_pages: stats.leaf_pages(),
                    branch_pages: stats.branch_pages(),
                    stored_leaf_bytes: stats.stored_bytes(),
                    metadata_bytes: stats.metadata_bytes(),
                    fragmented_bytes: stats.fragmented_bytes(),
                })
            }
            let mut tables = Vec::new();
            #(#plain_stats_getters)*
            #(#dict_stats_getters)*
            Ok(tables)
        }
    };

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            _entity_name: entity_name.clone(),
            tag: EndpointTag::MetaRead,
            fn_name: fn_name.clone(),
            params: vec![],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<Vec<TableInfo>>, AppError> {
                    #entity_name::#fn_name(&state.storage).map(AppJson)
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
