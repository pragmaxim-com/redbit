use crate::endpoint::EndpointDef;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod};
use crate::table::{DictTableDefs, IndexTableDefs, TableDef};
use proc_macro2::{Ident, Literal};
use quote::{format_ident, quote};
use syn::parse_quote;

pub fn table_info_fn(entity_name: &Ident, table_defs: &[TableDef], dict_table_defs: &[DictTableDefs], index_table_defs: &[IndexTableDefs]) -> FunctionDef {
    let plain_stats_getters = table_defs.iter().map(|td| {
        let db_name = td.var_name.to_string();
        let db_name_literal = Literal::string(&db_name.to_string());
        let table_name = &td.name;

        quote! {
            let db = index_dbs.get(#db_name_literal).cloned().unwrap();
            let db_arc = db.upgrade().ok_or_else(|| AppError::Custom("database closed".to_string()))?;
            let tx = db_arc.begin_read()?;
            let table = tx.open_table(#table_name)?;
            let stats = table.stats()?;
            let table_entries = table.len()?;
            tables.push(TableInfo::from_stats(#db_name_literal, table_entries, stats));
        }
    });
    let index_stats_getters = index_table_defs.iter().map(|defs| {
        let db_name = defs.var_name.clone();
        let db_name_literal = Literal::string(&db_name.to_string());
        let pk_by_index_table_name = &defs.pk_by_index.name;
        let index_by_pk_table_name = &defs.index_by_pk.name;

        quote! {
            let db = index_dbs.get(#db_name_literal).cloned().unwrap();
            let index_table = ReadOnlyIndexTable::new(db, #pk_by_index_table_name, #index_by_pk_table_name)?;
            tables.extend(index_table.stats()?);
        }
    });
    let dict_stats_getters = dict_table_defs.iter().map(|defs| {
        let db_name = defs.var_name.clone();
        let db_name_literal = Literal::string(&db_name.to_string());
        let dict_pk_to_ids_table_name = &defs.dict_pk_to_ids_table_def.name;
        let value_by_dict_pk_table_name = &defs.value_by_dict_pk_table_def.name;
        let value_to_dict_pk_table_name = &defs.value_to_dict_pk_table_def.name;
        let dict_pk_by_pk_table_name = &defs.dict_pk_by_pk_table_def.name;

        quote! {
            let db = index_dbs.get(#db_name_literal).cloned().unwrap();
            let dict_table = ReadOnlyDictTable::new(db, #dict_pk_to_ids_table_name, #value_by_dict_pk_table_name, #value_to_dict_pk_table_name, #dict_pk_by_pk_table_name)?;
            tables.extend(dict_table.stats()?);
        }
    });
    let fn_name = format_ident!("table_info");
    let fn_stream = quote! {
        pub fn #fn_name(storage: &Arc<Storage>) -> Result<Vec<TableInfo>, AppError> {
            let index_dbs = &storage.index_dbs;
            let mut tables = Vec::new();
            #(#plain_stats_getters)*
            #(#index_stats_getters)*
            #(#dict_stats_getters)*
            Ok(tables)
        }
    };

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: Some(parse_quote! { Vec<TableInfo> }),
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
