use crate::entity::info::TableInfoItem;
use crate::table::{DictTableDefs, IndexTableDefs, TableDef};
use proc_macro2::{Ident, Literal};
use quote::quote;

pub fn plain_table_info(column_name: &Ident, table_def: &TableDef) -> TableInfoItem {
    let definition = quote! { pub #column_name: TableInfo };

    let db_name = table_def.var_name.to_string();
    let db_name_literal = Literal::string(&db_name.to_string());
    let table_name = &table_def.name;

    let init =
        quote! {
            #column_name: {
                let db = index_dbs.get(#db_name_literal).cloned().unwrap();
                let db_arc = db.upgrade().ok_or_else(|| AppError::Custom("database closed".to_string()))?;
                let tx = db_arc.begin_read()?;
                let table = tx.open_table(#table_name)?;
                let stats = table.stats()?;
                let table_entries = table.len()?;
                TableInfo::from_stats(#db_name_literal, table_entries, stats)
            }
        };
    TableInfoItem { definition, init }
}

pub fn index_table_info(column_name: &Ident, index_table_defs: &IndexTableDefs) -> TableInfoItem {
    let db_name = index_table_defs.var_name.clone();
    let db_name_literal = Literal::string(&db_name.to_string());
    let pk_by_index_table_name = &index_table_defs.pk_by_index.name;
    let index_by_pk_table_name = &index_table_defs.index_by_pk.name;

    let definition = quote! { pub #column_name: Vec<TableInfo> };
    let init =
        quote! {
            #column_name: {
                let db = index_dbs.get(#db_name_literal).cloned().unwrap();
                let index_table = ReadOnlyIndexTable::new(db, #pk_by_index_table_name, #index_by_pk_table_name)?;
                index_table.stats()?
            }
        };
    TableInfoItem { definition, init }
}

pub fn dict_table_info(dict_table_defs: &DictTableDefs, column_name: &Ident) -> TableInfoItem {
    let db_name = dict_table_defs.var_name.clone();
    let db_name_literal = Literal::string(&db_name.to_string());
    let dict_pk_to_ids_table_name = &dict_table_defs.dict_pk_to_ids_table_def.name;
    let value_by_dict_pk_table_name = &dict_table_defs.value_by_dict_pk_table_def.name;
    let value_to_dict_pk_table_name = &dict_table_defs.value_to_dict_pk_table_def.name;
    let dict_pk_by_pk_table_name = &dict_table_defs.dict_pk_by_pk_table_def.name;

    let definition = quote! { pub #column_name: Vec<TableInfo> };
    let init =
        quote! {
            #column_name: {
                let db = index_dbs.get(#db_name_literal).cloned().unwrap();
                let dict_table = ReadOnlyDictTable::new(db, #dict_pk_to_ids_table_name, #value_by_dict_pk_table_name, #value_to_dict_pk_table_name, #dict_pk_by_pk_table_name)?;
                dict_table.stats()?
            }
        };
    TableInfoItem { definition, init }
}