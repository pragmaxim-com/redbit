use crate::rest::FunctionDef;
use crate::table::{DictTableDefs, IndexTableDefs, TableDef, TableType};
use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use syn::Type;

static TX_CONTEXT: &str = "TxContext";

pub enum TxType {
    Read,
    Write,
}

pub fn tx_context_name(tx_type: TxType) -> Ident {
    let suffix = TX_CONTEXT.to_string();
    let infix = match tx_type {
        TxType::Read => "Read",
        TxType::Write => "Write",
    };
    format_ident!("{}{}", infix, suffix)
}

pub fn entity_tx_context_type(entity_type: &Type, tx_type: TxType) -> Type {
    let entity_ident = match entity_type {
        Type::Path(p) => p.path.segments.last().unwrap().ident.clone(),
        _ => panic!("Unsupported entity type for tx context"),
    };
    let tx_context_type = format_ident!("{}{}", entity_ident, tx_context_name(tx_type));
    syn::parse_quote!(#tx_context_type)
}


#[derive(Clone)]
pub struct TxContextItem {
    pub write_definition: TokenStream,
    pub write_init: TokenStream,
    pub write_flush: Option<TokenStream>,
    pub read_definition: TokenStream,
    pub read_init: TokenStream,
}

pub fn write_tx_context(entity_tx_context_ty: &Type, tx_contexts: &[TxContextItem]) -> TokenStream {
    let definitions: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_definition.clone()).collect();
    let inits: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_init.clone()).collect();
    let flushes: Vec<TokenStream> = tx_contexts.iter().flat_map(|item| item.write_flush.clone()).collect();
    let write_tx_context_name = tx_context_name(TxType::Write);
    quote! {
        pub struct #entity_tx_context_ty<'txn> {
            #(#definitions),*
        }
        impl<'txn> #write_tx_context_name<'txn> for #entity_tx_context_ty<'txn> {
           fn begin_write_tx(plain_tx: &'txn WriteTransaction, index_dbs: &HashMap<String, Arc<Database>>) -> Result<Self, AppError> {
                Ok(Self {
                    #(#inits),*
                })
            }
           fn flush(self) -> Result<(), AppError> {
                #(#flushes);*;
                Ok(())
           }
        }
    }
}

pub fn read_tx_context(entity_tx_context_ty: &Type, tx_contexts: &[TxContextItem]) -> TokenStream {
    let definitions: Vec<TokenStream> = tx_contexts.iter().map(|item| item.read_definition.clone()).collect();
    let inits: Vec<TokenStream> = tx_contexts.iter().map(|item| item.read_init.clone()).collect();
    let read_tx_context_name = tx_context_name(TxType::Read);
    quote! {
        pub struct #entity_tx_context_ty {
            #(#definitions),*
        }
        impl #read_tx_context_name for #entity_tx_context_ty {
           fn begin_read_tx(storage: &Arc<Storage>) -> Result<Self, AppError> {
                let plain_tx = storage.plain_db.begin_read()?;
                Ok(Self {
                    #(#inits),*
                })
            }
        }
    }
}

pub fn tx_context(write_tx_context_ty: &Type, read_tx_context_ty: &Type, tx_contexts: &[TxContextItem]) -> TokenStream {
    let write_ctx = write_tx_context(write_tx_context_ty, tx_contexts);
    let read_ctx = read_tx_context(read_tx_context_ty, tx_contexts);
    quote! {
        #write_ctx
        #read_ctx
    }
}

pub fn tx_context_item(def: &TableDef) -> TxContextItem {
    let field_name = &def.var_name;
    let key_type = &def.key_type;
    let value_type = def.value_type.clone().unwrap_or_else(|| syn::parse_str::<Type>("()").unwrap());
    let table_name = &def.name;
    let (open_method, write_table_type, read_table_type) = match def.table_type {
        TableType::Index => (quote!(open_multimap_table), quote!(MultimapTable), quote!(ReadOnlyMultimapTable)),
        TableType::DictIndex | TableType::ValueByDictPk | TableType::ValueToDictPk | TableType::DictPkByPk => panic!("Dict tables cannot be here"),
        _ => (quote!(open_table), quote!(Table), quote!(ReadOnlyTable))
    };

    let write_definition =
        quote! {
            pub #field_name: #write_table_type<'txn, #key_type, #value_type>
        };

    let read_definition =
        quote! {
            pub #field_name: #read_table_type<#key_type, #value_type>
        };

    let init =
        quote! {
            #field_name: plain_tx.#open_method(#table_name)?
        };

    TxContextItem { write_definition, write_init: init.clone(), write_flush: None, read_definition, read_init: init }
}


pub fn tx_context_index_item(defs: &IndexTableDefs) -> TxContextItem {
    let var_name = &defs.var_name;
    let var_name_literal = Literal::string(&var_name.to_string());
    let key_type = &defs.key_type;
    let value_type = &defs.value_type;
    let index_by_pk_table_name = &defs.index_by_pk.name;
    let pk_by_index_table_name = &defs.pk_by_index.name;

    let write_definition =
        quote! {
            pub #var_name: TableWriter<#key_type, #value_type, IndexFactory<#key_type, #value_type>>
        };

    let read_definition =
        quote! {
            pub #var_name: ReadOnlyIndexTable<#key_type, #value_type>
        };

    let write_init =
        quote! {
            #var_name: TableWriter::new(
                index_dbs.get(#var_name_literal).cloned().ok_or_else(|| TableError::TableDoesNotExist(format!("Index table '{}' not found", #var_name_literal)))?,
                IndexFactory::new(
                    #pk_by_index_table_name,
                    #index_by_pk_table_name,
                ),
            )?
        };

    let write_flush = Some(quote! {
        self.#var_name.flush()?;
    });

    let read_init =
        quote! {
            #var_name: ReadOnlyIndexTable::new(
                storage.index_dbs.get(#var_name_literal).cloned().ok_or_else(|| TableError::TableDoesNotExist(format!("Index table '{}' not found", #var_name_literal)))?,
                #pk_by_index_table_name,
                #index_by_pk_table_name,
            )?
        };

    TxContextItem { write_definition, write_init, write_flush, read_definition, read_init }
}

pub fn tx_context_dict_item(defs: &DictTableDefs) -> TxContextItem {
    let var_name = &defs.var_name;
    let var_name_literal = Literal::string(&var_name.to_string());
    let key_type = &defs.key_type;
    let value_type = &defs.value_type;
    let dict_index_table_name = &defs.dict_index_table_def.name;
    let value_by_dict_pk_table_name = &defs.value_by_dict_pk_table_def.name;
    let value_to_dict_pk_table_name = &defs.value_to_dict_pk_table_def.name;
    let dict_pk_by_pk_table_name = &defs.dict_pk_by_pk_table_def.name;

    let write_definition =
        quote! {
            pub #var_name: TableWriter<#key_type, #value_type, DictFactory<#key_type, #value_type>>
        };

    let read_definition =
        quote! {
            pub #var_name: ReadOnlyDictTable<#key_type, #value_type>
        };

    let write_init =
        quote! {
            #var_name: TableWriter::new(
                index_dbs.get(#var_name_literal).cloned().ok_or_else(|| TableError::TableDoesNotExist(format!("Dict table '{}' not found", #var_name_literal)))?,
                DictFactory::new(
                    #dict_index_table_name,
                    #value_by_dict_pk_table_name,
                    #value_to_dict_pk_table_name,
                    #dict_pk_by_pk_table_name
                ),
            )?
        };

    let write_flush = Some(quote! {
        self.#var_name.flush()?;
    });

    let read_init =
        quote! {
            #var_name: ReadOnlyDictTable::new(
                storage.index_dbs.get(#var_name_literal).cloned().ok_or_else(|| TableError::TableDoesNotExist(format!("Dict table '{}' not found", #var_name_literal)))?,
                #dict_index_table_name,
                #value_by_dict_pk_table_name,
                #value_to_dict_pk_table_name,
                #dict_pk_by_pk_table_name
            )?
        };

    TxContextItem { write_definition, write_init, write_flush, read_definition, read_init }
}

pub fn tx_context_items(table_defs: &[TableDef]) -> Vec<TxContextItem> {
    table_defs
        .iter()
        .map(|def| tx_context_item(def))
        .collect()
}

pub fn begin_write_fn_def(tx_context_ty: &Type) -> FunctionDef {
    let fn_name = format_ident!("begin_write_tx");
    let fn_stream = quote! {
        pub fn #fn_name<'txn>(write_tx: &'txn WriteTransaction, index_dbs: &HashMap<String, Arc<Database>>) -> Result<#tx_context_ty<'txn>, AppError> {
            #tx_context_ty::begin_write_tx(write_tx, index_dbs)
        }
    };

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream: None,
        bench_stream: None,
    }

}

pub fn begin_read_fn_def(tx_context_ty: &Type) -> FunctionDef {
    let fn_name = format_ident!("begin_read_tx");
    let fn_stream = quote! {
        pub fn #fn_name(storage: &Arc<Storage>) -> Result<#tx_context_ty, AppError> {
            #tx_context_ty::begin_read_tx(storage)
        }
    };

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream: None,
        bench_stream: None,
    }

}