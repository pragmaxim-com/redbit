use proc_macro2::{Ident, TokenStream};
use quote::quote;
use crate::table::{DictTableDefs, TableDef};

pub struct StorageDef {
    pub db_defs: TokenStream,
    pub plain_table_stmnt: Vec<TokenStream>,
    pub dict_table_stmnt: Vec<TokenStream>,
}

pub fn get_db_defs(plain_table_defs: &[TableDef], dict_table_defs: &[DictTableDefs]) -> StorageDef {
    let plain_table_stmnt: Vec<TokenStream> =
        plain_table_defs.iter().map(|table_def| table_def.definition.clone()).collect();
    let dict_table_stmnt: Vec<TokenStream> =
        dict_table_defs.iter().flat_map(|dict_table_defs| dict_table_defs.all_table_defs().into_iter().map(|def| def.definition)).collect();

    let idents: Vec<Ident> = dict_table_defs.iter().map(|d| d.var_name.clone()).collect();
    let caches: Vec<usize> = dict_table_defs.iter().map(|d| d.db_cache).collect();

    let db_defs = quote! {
        pub fn db_defs() -> Vec<DbDef> {
            vec![#( DbDef { name: String::from(stringify!(#idents)), cache: #caches } ),*]
        }
    };
    StorageDef { db_defs, plain_table_stmnt, dict_table_stmnt }
}