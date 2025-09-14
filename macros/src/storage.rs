use proc_macro2::{Ident, TokenStream};
use quote::quote;
use crate::table::{DictTableDefs, IndexTableDefs, TableDef};

pub struct StorageDef {
    pub db_defs: TokenStream,
    pub table_defs: Vec<TokenStream>,
}

pub fn get_db_defs(plain_table_defs: &[TableDef], dict_table_defs: &[DictTableDefs], index_table_defs: &[IndexTableDefs]) -> StorageDef {
    let table_defs: Vec<TokenStream> = plain_table_defs
        .iter()
        .map(|table_def| table_def.definition.clone())
        .chain(index_table_defs.iter().flat_map(|defs| defs.all_table_defs().into_iter().map(|def| def.definition)))
        .chain(dict_table_defs.iter().flat_map(|defs| defs.all_table_defs().into_iter().map(|def| def.definition)))
        .collect();

    let idents: Vec<Ident> = index_table_defs.iter().map(|d| d.var_name.clone()).chain(dict_table_defs.iter().map(|d| d.var_name.clone())).collect();
    let caches: Vec<usize> = index_table_defs.iter().map(|d| d.db_cache).chain(dict_table_defs.iter().map(|d| d.db_cache)).collect();

    let db_defs = quote! {
        pub fn db_defs() -> Vec<DbDef> {
            vec![#( DbDef { name: String::from(stringify!(#idents)), cache: #caches } ),*]
        }
    };
    StorageDef { db_defs, table_defs }
}