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

#[derive(Clone)]
pub struct IndexTableDefs {
    pub(crate) var_name: Ident,
    pub(crate) key_type: Type,
    pub(crate) value_type: Type,
    #[allow(dead_code)]
    pub(crate) cache_weight: usize,
    pub(crate) pk_by_index: TableDef,
    pub(crate) index_by_pk: TableDef,
}

impl IndexTableDefs {
    pub fn new(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_name: &Ident, pk_type: &Type, cache_weight: usize) -> IndexTableDefs {
        let name = format_ident!("{}_{}_INDEX", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let var_name = Ident::new(&format!("{}", name).to_lowercase(), name.span());

        IndexTableDefs {
            var_name,
            key_type: pk_type.clone(),
            value_type: column_type.clone(),
            cache_weight,
            pk_by_index: TableDef::index_table_def(entity_name, column_name, column_type, pk_type),
            index_by_pk: TableDef::plain_table_def(entity_name, column_name, column_type, pk_name, pk_type),
        }
    }
    pub fn all_table_defs(&self) -> Vec<TableDef> {
        vec![
            self.pk_by_index.clone(),
            self.index_by_pk.clone(),
        ]
    }
}

#[derive(Clone)]
pub struct DictTableDefs {
    pub(crate) var_name: Ident,
    pub(crate) key_type: Type,
    pub(crate) value_type: Type,
    #[allow(dead_code)]
    pub(crate) cache_weight: usize,
    pub(crate) dict_index_table_def: TableDef,
    pub(crate) value_by_dict_pk_table_def: TableDef,
    pub(crate) value_to_dict_pk_table_def: TableDef,
    pub(crate) dict_pk_by_pk_table_def: TableDef,
}

impl DictTableDefs {
    pub fn new(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_name: &Ident, pk_type: &Type, cache_weight: usize) -> DictTableDefs {
        let name = format_ident!("{}_{}_DICT", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let var_name = Ident::new(&format!("{}", name).to_lowercase(), name.span());

        DictTableDefs {
            var_name,
            key_type: pk_type.clone(),
            value_type: column_type.clone(),
            cache_weight,
            dict_index_table_def: TableDef::dict_index_table_def(entity_name, column_name, pk_type),
            value_by_dict_pk_table_def: TableDef::value_by_dict_pk_table_def(entity_name, column_name, column_type, pk_type),
            value_to_dict_pk_table_def: TableDef::value_to_dict_pk_table_def(entity_name, column_name, column_type, pk_type),
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



#[derive(Clone)]
pub struct TableDef {
    pub name: Ident,
    pub var_name: Ident,
    pub key_type: Type,
    pub value_type: Option<Type>,
    pub cache_weight: usize,
    pub table_type: TableType,
    pub definition: TokenStream,
}

impl TableDef {
    pub fn pk(entity_name: &Ident, pk_name: &Ident, pk_type: &Type, cache_weight: usize) -> TableDef {
        let name = format_ident!("{}_{}", entity_name.to_string().to_uppercase(), pk_name.to_string().to_uppercase());
        let name_str = name.to_string();
        let var_name = Ident::new(&format!("{}", name).to_lowercase(), name.span());
        let definition =
            quote! {
                pub const #name: TableDefinition<'static, #pk_type, ()> = TableDefinition::new(#name_str);
            };
        TableDef {
            name,
            var_name,
            cache_weight,
            key_type: pk_type.clone(),
            value_type: None,
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
        let var_name = Ident::new(&format!("{}", name).to_lowercase(), name.span());
        let definition = quote! {
            pub const #name: TableDefinition<'static, #pk_type, #column_type> = TableDefinition::new(#name_str);
        };
        TableDef {
            name,
            var_name,
            key_type: pk_type.clone(),
            value_type: Some(column_type.clone()),
            cache_weight: 0,
            table_type: TableType::Plain,
            definition
        }
    }
    
    pub fn index_table_def(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}_INDEX", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let var_name = Ident::new(&format!("{}", name).to_lowercase(), name.span());
        let name_str = &name.to_string();
        let definition = quote! {
            pub const #name: MultimapTableDefinition<'static, #column_type, #pk_type> = MultimapTableDefinition::new(#name_str);
        };
        TableDef {
            name,
            var_name,
            key_type: column_type.clone(),
            value_type: Some(pk_type.clone()),
            cache_weight: 0,
            table_type: TableType::Index,
            definition
        }
    }

    pub fn dict_index_table_def(entity_name: &Ident, column_name: &Ident, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}_DICT_INDEX", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let var_name = Ident::new(&format!("{}", name).to_lowercase(), name.span());
        let name_str = &name.to_string();
        let definition =
        quote! {
            pub const #name: MultimapTableDefinition<'static, #pk_type, #pk_type>= MultimapTableDefinition::new(#name_str);
        };
        TableDef {
            name,
            var_name,
            key_type: pk_type.clone(),
            value_type: Some(pk_type.clone()),
            cache_weight: 0,
            table_type: TableType::DictIndex,
            definition
        }
    }

    pub fn value_by_dict_pk_table_def(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}_BY_DICT_PK", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let var_name = Ident::new(&format!("{}", name).to_lowercase(), name.span());
        let name_str = &name.to_string();
        let definition =
            quote! {
            pub const #name: TableDefinition<'static, #pk_type, #column_type> = TableDefinition::new(#name_str);
        };
        TableDef {
            name,
            var_name,
            key_type: pk_type.clone(),
            value_type: Some(column_type.clone()),
            cache_weight: 0,
            table_type: TableType::ValueByDictPk,
            definition
        }
    }

    pub fn value_to_dict_pk_table_def(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}_TO_DICT_PK", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let var_name = Ident::new(&format!("{}", name).to_lowercase(), name.span());
        let name_str = &name.to_string();
        let definition = quote! {
            pub const #name: TableDefinition<'static, #column_type, #pk_type> = TableDefinition::new(#name_str);
        };

        TableDef {
            name,
            var_name,
            key_type: column_type.clone(),
            value_type: Some(pk_type.clone()),
            cache_weight: 0,
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
        let var_name = Ident::new(&format!("{}", name).to_lowercase(), name.span());
        let name_str = &name.to_string();
        let definition = quote! {
            pub const #name: TableDefinition<'static, #pk_type, #pk_type> = TableDefinition::new(#name_str);
        };
        TableDef {
            name,
            var_name,
            key_type: pk_type.clone(),
            value_type: Some(pk_type.clone()),
            cache_weight: 0,
            table_type: TableType::DictPkByPk,
            definition
        }
    }

}
