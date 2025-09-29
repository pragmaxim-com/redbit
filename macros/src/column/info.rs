use crate::entity::info::TableInfoItem;
use crate::table::{DictTableDefs, IndexTableDefs, PlainTableDef};
use proc_macro2::{Ident, Literal};
use quote::quote;

pub fn plain_table_info(column_name: &Ident, table_def: &PlainTableDef) -> TableInfoItem {
    let definition = quote! { pub #column_name: TableInfo };
    let name_lit   = Literal::string(&table_def.var_name.to_string());
    let table_name = &table_def.underlying.name;
    let shards     = table_def.column_props.shards;
    let is_sharded = shards >= 2;

    let init = if is_sharded {
        quote! {
            #column_name: {
                let ro = ShardedReadOnlyPlainTable::new(
                    BytesPartitioner::new(#shards),
                    storage.fetch_sharded_dbs(#name_lit, Some(#shards))?,
                    #table_name
                )?;
                ro.stats()?
            }
        }
    } else {
        quote! {
            #column_name: {
                let ro = ReadOnlyPlainTable::new(storage.fetch_single_db(#name_lit)?, #table_name)?;
                ro.stats()?
            }
        }
    };

    TableInfoItem { definition, init }
}

// ----------------- Index -----------------

pub fn index_table_info(column_name: &Ident, defs: &IndexTableDefs) -> TableInfoItem {
    let definition = quote! { pub #column_name: Vec<TableInfo> };

    let name_lit    = Literal::string(&defs.var_name.to_string());
    let pk_by_index = &defs.pk_by_index.name;
    let index_by_pk = &defs.index_by_pk.name;
    let shards      = defs.column_props.shards;
    let is_sharded  = shards >= 2;

    let init = if is_sharded {
        quote! {
            #column_name: {
                let ro = ShardedReadOnlyIndexTable::new(
                    BytesPartitioner::new(#shards),
                    storage.fetch_sharded_dbs(#name_lit, Some(#shards))?,
                    #pk_by_index,
                    #index_by_pk
                )?;
                ro.stats()?
            }
        }
    } else {
        quote! {
            #column_name: {
                let ro = ReadOnlyIndexTable::new(storage.fetch_single_db(#name_lit)?, #pk_by_index, #index_by_pk)?;
                ro.stats()?
            }
        }
    };

    TableInfoItem { definition, init }
}

// ----------------- Dict -----------------

pub fn dict_table_info(defs: &DictTableDefs, column_name: &Ident) -> TableInfoItem {
    let definition = quote! { pub #column_name: Vec<TableInfo> };
    let name_lit        = Literal::string(&defs.var_name.to_string());
    let dict_pk_to_ids  = &defs.dict_pk_to_ids_table_def.name;
    let value_by_dict   = &defs.value_by_dict_pk_table_def.name;
    let value_to_dict   = &defs.value_to_dict_pk_table_def.name;
    let dict_pk_by_pk   = &defs.dict_pk_by_pk_table_def.name;
    let shards           = defs.column_props.shards;
    let is_sharded        = shards >= 2;

    let init = if is_sharded {
        quote! {
            #column_name: {
                let ro = ShardedReadOnlyDictTable::new(
                    Xxh3Partitioner::new(#shards),
                    storage.fetch_sharded_dbs(#name_lit, Some(#shards))?,
                    #dict_pk_to_ids,
                    #value_by_dict,
                    #value_to_dict,
                    #dict_pk_by_pk
                )?;
                ro.stats()?
            }
        }
    } else {
        quote! {
            #column_name: {
                let ro = ReadOnlyDictTable::new(
                    storage.fetch_single_db(#name_lit)?,
                    #dict_pk_to_ids,
                    #value_by_dict,
                    #value_to_dict,
                    #dict_pk_by_pk
                )?;
                ro.stats()?
            }
        }
    };

    TableInfoItem { definition, init }
}
