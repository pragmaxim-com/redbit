use crate::table::{DictTableDefs, IndexTableDefs, PlainTableDef};
use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub struct StorageDef {
    pub db_defs: TokenStream,
    pub table_defs: Vec<TokenStream>,
}

pub fn get_db_defs(plain_table_defs: &[PlainTableDef], dict_table_defs: &[DictTableDefs], index_table_defs: &[IndexTableDefs]) -> StorageDef {
    let table_defs: Vec<TokenStream> = plain_table_defs
        .iter()
        .map(|table_def| table_def.underlying.definition.clone())
        .chain(index_table_defs.iter().flat_map(|defs| defs.all_table_defs().into_iter().map(|def| def.definition)))
        .chain(dict_table_defs.iter().flat_map(|defs| defs.all_table_defs().into_iter().map(|def| def.definition)))
        .collect();

    let idents: Vec<Ident> =
        plain_table_defs.iter().map(|d| d.var_name.clone())
            .chain(index_table_defs.iter().map(|d| d.var_name.clone()))
            .chain(dict_table_defs.iter().map(|d| d.var_name.clone()))
            .collect();
    let db_caches: Vec<usize> =
        plain_table_defs.iter().map(|d| d.column_props.db_cache_weight)
            .chain(index_table_defs.iter().map(|d| d.column_props.db_cache_weight))
            .chain(dict_table_defs.iter().map(|d| d.column_props.db_cache_weight))
            .collect();
    
    let lru_cache_sizes: Vec<usize> =
        plain_table_defs.iter().map(|d| d.column_props.lru_cache_size)
            .chain(index_table_defs.iter().map(|d| d.column_props.lru_cache_size))
            .chain(dict_table_defs.iter().map(|d| d.column_props.lru_cache_size))
            .collect();

    let shards: Vec<usize> =
        plain_table_defs.iter().map(|d| d.column_props.shards)
            .chain(index_table_defs.iter().map(|d| d.column_props.shards))
            .chain(dict_table_defs.iter().map(|d| d.column_props.shards))
            .collect();


    let db_defs = quote! {
        pub fn db_defs() -> Vec<DbDef> {
            vec![#( DbDef { name: String::from(stringify!(#idents)), shards: #shards, db_cache_weight_or_zero: #db_caches, lru_cache_size_or_zero: #lru_cache_sizes } ),*]
        }
    };
    StorageDef { db_defs, table_defs }
}