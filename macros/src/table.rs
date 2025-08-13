use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;

#[derive(Clone, Debug, strum_macros::Display)]
pub enum TableType {
    Pk,
    Plain,
    Index,
    DictIndex,
    ValueByDictPk,
    ValueToDictPk,
    DictPkByPk,
}

#[derive(Clone, Debug)]
pub struct DictTableDefs {
    pub(crate) value_to_dict_pk_cache: Option<Ident>,
    pub(crate) dict_index_table_def: TableDef,
    pub(crate) value_by_dict_pk_table_def: TableDef,
    pub(crate) value_to_dict_pk_table_def: TableDef,
    pub(crate) dict_pk_by_pk_table_def: TableDef,
}

impl DictTableDefs {
    pub fn new(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_name: &Ident, pk_type: &Type, cache_size: Option<usize>) -> DictTableDefs {
        let value_to_dict_pk_table_def = TableDef::value_to_dict_pk_table_def(entity_name, column_name, column_type, pk_type, cache_size);
        DictTableDefs {
            value_to_dict_pk_cache: value_to_dict_pk_table_def.cache.clone().map(|c|c.0),
            dict_index_table_def: TableDef::dict_index_table_def(entity_name, column_name, pk_type),
            value_by_dict_pk_table_def: TableDef::value_by_dict_pk_table_def(entity_name, column_name, column_type, pk_type),
            value_to_dict_pk_table_def,
            dict_pk_by_pk_table_def: TableDef::dict_pk_by_pk_table_def(entity_name, column_name, pk_name, pk_type),
        }
    }
    pub fn all_table_defs(&self) -> Vec<TableDef> {
        vec![
            self.dict_index_table_def.clone(),
            self.value_by_dict_pk_table_def.clone(),
            self.value_to_dict_pk_table_def.clone(),
            self.dict_pk_by_pk_table_def.clone(),
        ]
    }
}

#[derive(Clone, Debug)]
pub struct TableDef {
    pub name: Ident,
    pub cache: Option<(Ident, TokenStream)>,
    pub table_type: TableType,
    pub definition: TokenStream,
}

impl TableDef {
    pub fn pk(entity_name: &Ident, pk_name: &Ident, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}", entity_name.to_string().to_uppercase(), pk_name.to_string().to_uppercase());
        let name_str = name.to_string();
        let definition =
            quote! {
                pub const #name: TableDefinition<'static, Bincode<#pk_type>, ()> = TableDefinition::new(#name_str);
            };
        TableDef {
            name,
            cache: None,
            table_type: TableType::Pk,
            definition
        }
    }

    pub fn plain_table_def(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_name: &Ident, pk_type: &Type) -> TableDef {
        let name = format_ident!(
        "{}_{}_BY_{}",
        entity_name.to_string().to_uppercase(),
        column_name.to_string().to_uppercase(),
        pk_name.to_string().to_uppercase()
    );
        let name_str = &name.to_string();
        let definition = quote! {
            pub const #name: TableDefinition<'static, Bincode<#pk_type>, Bincode<#column_type>> = TableDefinition::new(#name_str);
        };
        TableDef {
            name,
            cache: None,
            table_type: TableType::Plain,
            definition
        }
    }
    
    pub fn index_table_def(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}_INDEX", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let name_str = &name.to_string();
        let definition = quote! {
            pub const #name: MultimapTableDefinition<'static, Bincode<#column_type>, Bincode<#pk_type>> = MultimapTableDefinition::new(#name_str);
        };
        TableDef {
            name,
            cache: None,
            table_type: TableType::Index,
            definition
        }
    }

    pub fn dict_index_table_def(entity_name: &Ident, column_name: &Ident, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}_DICT_INDEX", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let name_str = &name.to_string();
        let definition =
        quote! {
            pub const #name: MultimapTableDefinition<'static, Bincode<#pk_type>, Bincode<#pk_type>>= MultimapTableDefinition::new(#name_str);
        };
        TableDef {
            name,
            cache: None,
            table_type: TableType::DictIndex,
            definition
        }
    }

    pub fn value_by_dict_pk_table_def(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}_BY_DICT_PK", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let name_str = &name.to_string();
        let definition =
            quote! {
            pub const #name: TableDefinition<'static, Bincode<#pk_type>, Bincode<#column_type>> = TableDefinition::new(#name_str);
        };
        TableDef {
            name,
            cache: None,
            table_type: TableType::ValueByDictPk,
            definition
        }
    }

    pub fn value_to_dict_pk_table_def(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_type: &Type, cache_size_opt: Option<usize>) -> TableDef {
        let name = format_ident!("{}_{}_TO_DICT_PK", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let name_str = &name.to_string();
        let definition = quote! {
            pub const #name: TableDefinition<'static, Bincode<#column_type>, Bincode<#pk_type>> = TableDefinition::new(#name_str);
        };

        let cache = cache_size_opt.map(|cache_size| {
            let cache_name = format_ident!("{}_CACHE", name);
            let cache_name_str = &cache_name.to_string();
            let cache_definition = quote! {
                pub static #cache_name: CacheDef<#column_type, #pk_type> =
                        CacheDef::new(#cache_name_str, std::num::NonZeroUsize::new(#cache_size).expect("cache size must be > 0"));
            };
            (cache_name, cache_definition)
        });

        TableDef {
            name,
            cache,
            table_type: TableType::ValueToDictPk,
            definition,
        }
    }

    pub fn dict_pk_by_pk_table_def(entity_name: &Ident, column_name: &Ident, pk_name: &Ident, pk_type: &Type) -> TableDef {
        let name = format_ident!(
            "{}_{}_DICT_PK_BY_{}",
            entity_name.to_string().to_uppercase(),
            column_name.to_string().to_uppercase(),
            pk_name.to_string().to_uppercase()
        );
        let name_str = &name.to_string();
        let definition = quote! {
            pub const #name: TableDefinition<'static, Bincode<#pk_type>, Bincode<#pk_type>> = TableDefinition::new(#name_str);
        };
        TableDef {
            name,
            cache: None,
            table_type: TableType::DictPkByPk,
            definition
        }
    }

}
