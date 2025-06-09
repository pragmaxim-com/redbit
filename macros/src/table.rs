use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;

#[derive(Clone, Debug)]
pub struct TableDef {
    pub name: Ident,
    pub definition: TokenStream,
}

impl TableDef {
    pub fn pk(entity_name: &Ident, pk_name: &Ident, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}", entity_name.to_string().to_uppercase(), pk_name.to_string().to_uppercase());
        let name_str = name.to_string();
        let definition =
            quote! {
            pub const #name: ::redb::TableDefinition<'static, Bincode<#pk_type>, ()> = ::redb::TableDefinition::new(#name_str);
        };
        TableDef {
            name,
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
        pub const #name: ::redb::TableDefinition<'static, Bincode<#pk_type>, Bincode<#column_type>> = ::redb::TableDefinition::new(#name_str);
    };
        TableDef {
            name,
            definition
        }
    }
    
    pub fn index_table_def(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}_INDEX", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let name_str = &name.to_string();
        let definition = quote! {
        pub const #name: ::redb::MultimapTableDefinition<'static, Bincode<#column_type>, Bincode<#pk_type>> = ::redb::MultimapTableDefinition::new(#name_str);
    };
        TableDef {
            name,
            definition
        }
    }

    pub fn dict_index_table_def(entity_name: &Ident, column_name: &Ident, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}_DICT_INDEX", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let name_str = &name.to_string();
        let definition =
            quote! {
            pub const #name: ::redb::MultimapTableDefinition<'static, Bincode<#pk_type>, Bincode<#pk_type>>= ::redb::MultimapTableDefinition::new(#name_str);
        };
        TableDef {
            name,
            definition
        }
    }

    pub fn value_by_dict_pk_table_def(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}_BY_DICT_PK", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let name_str = &name.to_string();
        let definition =
            quote! {
            pub const #name: ::redb::TableDefinition<'static, Bincode<#pk_type>, Bincode<#column_type>> = ::redb::TableDefinition::new(#name_str);
        };
        TableDef {
            name,
            definition
        }
    }

    pub fn value_to_dict_pk_table_def(entity_name: &Ident, column_name: &Ident, column_type: &Type, pk_type: &Type) -> TableDef {
        let name = format_ident!("{}_{}_TO_DICT_PK", entity_name.to_string().to_uppercase(), column_name.to_string().to_uppercase());
        let name_str = &name.to_string();
        let definition = quote! {
        pub const #name: ::redb::TableDefinition<'static, Bincode<#column_type>, Bincode<#pk_type>> = ::redb::TableDefinition::new(#name_str);
    };
        TableDef {
            name,
            definition
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
        pub const #name: ::redb::TableDefinition<'static, Bincode<#pk_type>, Bincode<#pk_type>> = ::redb::TableDefinition::new(#name_str);
    };
        TableDef {
            name,
            definition
        }
    }

}
