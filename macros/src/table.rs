use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;
use crate::field_parser::{ColumnProps, EntityDef, UsedBy};

#[derive(Clone, Debug, strum_macros::Display)]
pub enum TableType {
    Pk,
    Plain,
    Index,
    DictPkToIds,
    ValueByDictPk,
    ValueToDictPk,
    DictPkByPk,
}

#[derive(Clone)]
pub struct PlainTableDef {
    pub(crate) root_pk: bool,
    pub(crate) var_name: Ident,
    pub(crate) key_type: Type,
    pub(crate) value_type: Option<Type>,
    pub(crate) column_props: ColumnProps,
    pub(crate) underlying: TableDef,
    pub(crate) used_by: Option<UsedBy>,
}

impl PlainTableDef {
    pub fn new(underlying: TableDef, column_props: ColumnProps, used_by: Option<UsedBy>, root_pk: bool) -> PlainTableDef {
        PlainTableDef {
            root_pk,
            var_name: underlying.var_name.clone(),
            key_type: underlying.key_type.clone(),
            value_type: underlying.value_type.clone(),
            column_props,
            underlying,
            used_by
        }
    }
}


#[derive(Clone)]
pub struct IndexTableDefs {
    pub(crate) var_name: Ident,
    pub(crate) key_type: Type,
    pub(crate) value_type: Type,
    pub(crate) column_props: ColumnProps,
    pub(crate) used_by: Option<UsedBy>,
    pub(crate) pk_by_index: TableDef,
    pub(crate) index_by_pk: TableDef,
}

impl IndexTableDefs {
    pub fn new(entity_def: &EntityDef, column_name: &Ident, column_type: &Type, column_props: ColumnProps, used_by: Option<UsedBy>) -> IndexTableDefs {
        let entity_name = &entity_def.entity_name;
        let pk_type = &entity_def.key_def.field_def().tpe;
        let name = format_ident!("{}_{}_INDEX", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let var_name = Ident::new(&format!("{}", name).to_lowercase(), name.span());

        IndexTableDefs {
            var_name,
            key_type: pk_type.clone(),
            value_type: column_type.clone(),
            column_props,
            used_by,
            pk_by_index: TableDef::index_table_def(entity_def, column_name, column_type),
            index_by_pk: TableDef::plain_table_def(entity_def, column_name, column_type),
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
    pub(crate) column_props: ColumnProps,
    pub(crate) used_by: Option<UsedBy>,
    pub(crate) dict_pk_to_ids_table_def: TableDef,
    pub(crate) value_by_dict_pk_table_def: TableDef,
    pub(crate) value_to_dict_pk_table_def: TableDef,
    pub(crate) dict_pk_by_pk_table_def: TableDef,
}

impl DictTableDefs {
    pub fn new(entity_def: &EntityDef, column_name: &Ident, column_type: &Type, column_props: ColumnProps, used_by: Option<UsedBy>) -> DictTableDefs {
        let entity_name = &entity_def.entity_name;
        let key_def = &entity_def.key_def.field_def();
        let pk_name = &key_def.name;
        let pk_type = &key_def.tpe;
        let name = format_ident!("{}_{}_DICT", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let var_name = Ident::new(&format!("{}", name).to_lowercase(), name.span());

        DictTableDefs {
            var_name,
            key_type: pk_type.clone(),
            value_type: column_type.clone(),
            column_props,
            used_by,
            dict_pk_to_ids_table_def: TableDef::dict_pk_to_ids_table_def(entity_name, column_name, pk_type),
            value_by_dict_pk_table_def: TableDef::value_by_dict_pk_table_def(entity_name, column_name, column_type, pk_type),
            value_to_dict_pk_table_def: TableDef::value_to_dict_pk_table_def(entity_name, column_name, column_type, pk_type),
            dict_pk_by_pk_table_def: TableDef::dict_pk_by_pk_table_def(entity_name, column_name, pk_name, pk_type),
        }
    }
    pub fn all_table_defs(&self) -> Vec<TableDef> {
        vec![
            self.dict_pk_to_ids_table_def.clone(),
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
    pub _table_type: TableType,
    pub definition: TokenStream,
}

impl TableDef {
    pub fn pk(entity_def: &EntityDef) -> TableDef {
        let entity_name = &entity_def.entity_name;
        let key_def = &entity_def.key_def.field_def();
        let pk_name = &key_def.name;
        let pk_type = &key_def.tpe;
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
            key_type: pk_type.clone(),
            value_type: None,
            _table_type: TableType::Pk,
            definition
        }
    }

    pub fn plain_table_def(entity_def: &EntityDef, column_name: &Ident, column_type: &Type) -> TableDef {
        let entity_name = &entity_def.entity_name;
        let key_def = &entity_def.key_def.field_def();
        let pk_name = &key_def.name;
        let pk_type = &key_def.tpe;
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
            _table_type: TableType::Plain,
            definition
        }
    }
    
    pub fn index_table_def(entity_def: &EntityDef, column_name: &Ident, column_type: &Type) -> TableDef {
        let entity_name = &entity_def.entity_name;
        let pk_type = &entity_def.key_def.field_def().tpe;
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
            _table_type: TableType::Index,
            definition
        }
    }

    pub fn dict_pk_to_ids_table_def(entity_name: &Ident, column_name: &Ident, pk_type: &Type) -> TableDef {
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
            _table_type: TableType::DictPkToIds,
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
            _table_type: TableType::ValueByDictPk,
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
            _table_type: TableType::ValueToDictPk,
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
            _table_type: TableType::DictPkByPk,
            definition
        }
    }

}
