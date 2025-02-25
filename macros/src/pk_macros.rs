use crate::Pk;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

pub struct PkMacros {
    pub table_definition: TokenStream,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub functions: Vec<(String, TokenStream)>,
}

impl PkMacros {
    pub fn new(struct_name: &Ident, pk_column: &Pk) -> Self {
        let table_ident = format_ident!("{}_{}", struct_name.to_string().to_uppercase(), pk_column.field.name.to_string().to_uppercase());
        let table_name_str = table_ident.to_string();
        let pk_name: Ident = pk_column.field.name.clone();
        let pk_type = pk_column.field.tpe.clone();

        let table_definition = quote! {
            pub const #table_ident: ::redb::TableDefinition<'static, Bincode<#pk_type>, ()> = ::redb::TableDefinition::new(#table_name_str);
        };

        let store_statement = quote! {
            let mut table = write_tx.open_table(#table_ident)?;
            table.insert(&instance.#pk_name, ())?;
        };

        let store_many_statement = quote! {
            let mut table = write_tx.open_table(#table_ident)?;
            for instance in instances.iter() {
                table.insert(&instance.#pk_name, ())?;
            };
        };

        let delete_statement = quote! {
            let mut table = write_tx.open_table(#table_ident)?;
            let value = table.remove(pk)?;
            value.map(|g| g.value());
        };

        let mut functions: Vec<(String, TokenStream)> = Vec::new();
        let get_fn_name = format_ident!("get");
        functions.push((
            get_fn_name.to_string(),
            quote! {
                pub fn #get_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<Option<#struct_name>, DbEngineError> {
                    let table = read_tx.open_table(#table_ident)?;
                    if table.get(pk)?.is_some() {
                        Ok(Some(Self::compose(&read_tx, pk)?))
                    } else {
                        Ok(None)
                    }
                }
            },
        ));

        let all_fn_name = format_ident!("all");
        functions.push((
            all_fn_name.to_string(),
            quote! {
                pub fn #all_fn_name(read_tx: &::redb::ReadTransaction) -> Result<Vec<#struct_name>, DbEngineError> {
                    let table = read_tx.open_table(#table_ident)?;
                    let mut iter = table.iter()?;
                    let mut results = Vec::new();
                    while let Some(entry_res) = iter.next() {
                        let pk = entry_res?.0.value();
                        results.push(Self::compose(&read_tx, &pk)?);
                    }
                    Ok(results)
                }
            },
        ));

        let first_fn_name = format_ident!("first");
        functions.push((
            first_fn_name.to_string(),
            quote! {
                pub fn #first_fn_name(read_tx: &::redb::ReadTransaction) -> Result<Option<#struct_name>, DbEngineError> {
                    let table = read_tx.open_table(#table_ident)?;
                    if let Some((k, _)) = table.last()? {
                        return Self::compose(&read_tx, &k.value()).map(Some);
                    }
                    Ok(None)
                }
            },
        ));

        let last_fn_name = format_ident!("last");
        functions.push((
            last_fn_name.to_string(),
            quote! {
                pub fn #last_fn_name(read_tx: &::redb::ReadTransaction) -> Result<Option<#struct_name>, DbEngineError> {
                    let table = read_tx.open_table(#table_ident)?;
                    if let Some((k, _)) = table.last()? {
                        return Self::compose(&read_tx, &k.value()).map(Some);
                    }
                    Ok(None)
                }
            },
        ));

        if pk_column.range {
            let range_fn_name = format_ident!("range");
            functions.push((range_fn_name.to_string(), quote! {
                pub fn #range_fn_name(read_tx: &::redb::ReadTransaction, from: &#pk_type, until: &#pk_type) -> Result<Vec<#struct_name>, DbEngineError> {
                    let table = read_tx.open_table(#table_ident)?;
                    let range = from.clone()..until.clone();
                    let mut iter = table.range(range)?;
                    let mut results = Vec::new();
                    while let Some(entry_res) = iter.next() {
                        let pk = entry_res?.0.value();
                        results.push(Self::compose(&read_tx, &pk)?);
                    }
                    Ok(results)
                }
            }));
            let pk_range_fn_name = format_ident!("pk_range");
            functions.push((pk_range_fn_name.to_string(), quote! {
                fn #pk_range_fn_name(write_tx: &::redb::WriteTransaction, from: &#pk_type, until: &#pk_type) -> Result<Vec<#pk_type>, DbEngineError> {
                    let table = write_tx.open_table(#table_ident)?;
                    let range = from.clone()..until.clone();
                    let mut iter = table.range(range)?;
                    let mut results = Vec::new();
                    while let Some(entry_res) = iter.next() {
                        let pk = entry_res?.0.value();
                        results.push(pk);
                    }
                    Ok(results)
                }
            }))
        };

        PkMacros { table_definition, store_statement, store_many_statement, delete_statement, functions }
    }
}
