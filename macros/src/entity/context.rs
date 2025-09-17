use crate::rest::FunctionDef;
use crate::table::{DictTableDefs, IndexTableDefs, TableDef};
use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use syn::Type;

pub static TX_CONTEXT: &str = "TxContext";

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
    pub write_constructor: TokenStream,
    pub write_begin: TokenStream,
    pub write_flush: Option<TokenStream>,
    pub write_shutdown: TokenStream,
    pub read_definition: TokenStream,
    pub read_constructor: TokenStream,
}

pub fn write_tx_context(entity_tx_context_ty: &Type, tx_contexts: &[TxContextItem]) -> TokenStream {
    let definitions: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_definition.clone()).collect();
    let constructors: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_constructor.clone()).collect();
    let begins: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_begin.clone()).collect();
    let shutdowns: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_shutdown.clone()).collect();
    let all_flushes: Vec<TokenStream> = tx_contexts.iter().flat_map(|item| item.write_flush.clone()).collect();
    let pk_flush = all_flushes.first().unwrap();
    let tail_flushes: Vec<TokenStream> = all_flushes.clone().into_iter().skip(1).collect();
    let write_tx_context_name = tx_context_name(TxType::Write);
    quote! {
        pub struct #entity_tx_context_ty {
            #(#definitions),*
        }
        impl #write_tx_context_name for #entity_tx_context_ty {
           fn new_write_ctx(storage: &Arc<Storage>) -> Result<Self, AppError> {
                Ok(Self {
                    #(#constructors),*
                })
            }
           fn begin_writing(&self) -> Result<(), AppError> {
                #(#begins;)*
                Ok(())
            }
           fn stop_writing(self) -> Result<(), AppError> {
                #(#shutdowns;)*
                Ok(())
            }
           fn commit_ctx_async(&self) -> Result<Vec<FlushFuture>, AppError> {
                let mut futures: Vec<FlushFuture> = Vec::new();
                #( futures.extend(#all_flushes); )*
                Ok(futures)
           }
           fn two_phase_commit(&self) -> Result<(), AppError> {
                let mut futures = Vec::new();
                #( futures.extend(#tail_flushes); )*
                let _ = futures.into_iter().map(|f| f.wait()).collect::<Result<Vec<_>, _>>()?;
                #pk_flush.into_iter().try_for_each(|f| f.wait())?;
                Ok(())
            }
        }
    }
}

pub fn read_tx_context(entity_tx_context_ty: &Type, tx_contexts: &[TxContextItem]) -> TokenStream {
    let definitions: Vec<TokenStream> = tx_contexts.iter().map(|item| item.read_definition.clone()).collect();
    let constructors: Vec<TokenStream> = tx_contexts.iter().map(|item| item.read_constructor.clone()).collect();
    let read_tx_context_name = tx_context_name(TxType::Read);
    quote! {
        pub struct #entity_tx_context_ty {
            #(#definitions),*
        }
        impl #read_tx_context_name for #entity_tx_context_ty {
           fn begin_read_ctx(storage: &Arc<Storage>) -> Result<Self, AppError> {
                Ok(Self {
                    #(#constructors),*
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

pub fn tx_context_plain_item(def: &TableDef) -> TxContextItem {
    let var_name = &def.var_name;
    let var_name_literal = Literal::string(&var_name.to_string());
    let key_type = &def.key_type;
    let value_type = def.value_type.clone().unwrap_or_else(|| syn::parse_str::<Type>("()").unwrap());
    let table_name = &def.name;

    let write_definition =
        quote! {
            pub #var_name: TableWriter<#key_type, #value_type, PlainFactory<#key_type, #value_type>>
        };

    let read_definition =
        quote! {
            pub #var_name: ReadOnlyTable<#key_type, #value_type>
        };

    let write_constructor =
        quote! {
            #var_name: TableWriter::new(
                storage.index_dbs.get(#var_name_literal).cloned().ok_or_else(|| TableError::TableDoesNotExist(format!("Plain table '{}' not found", #var_name_literal)))?,
                PlainFactory::new(#table_name),
            )?
        };

    let write_begin = quote! {
        let _ = &self.#var_name.begin()?
    };

    let write_flush = Some(quote! {
        vec![self.#var_name.flush_async()?]
    });

    let write_shutdown = quote! {
        let _ = self.#var_name.shutdown()?
    };

    let read_constructor =
        quote! {
            #var_name: {
                let index_db = storage.index_dbs.get(#var_name_literal).cloned().ok_or_else(|| TableError::TableDoesNotExist(format!("Plain table '{}' not found", #var_name_literal)))?;
                let plain_tx = index_db.begin_read()?;
                plain_tx.open_table(#table_name)?
            }
        };

    TxContextItem { write_definition, write_constructor, write_begin, write_flush, write_shutdown, read_definition, read_constructor }
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

    let write_constructor =
        quote! {
            #var_name: TableWriter::new(
                storage.index_dbs.get(#var_name_literal).cloned().ok_or_else(|| TableError::TableDoesNotExist(format!("Index table '{}' not found", #var_name_literal)))?,
                IndexFactory::new(
                    #pk_by_index_table_name,
                    #index_by_pk_table_name,
                ),
            )?
        };

    let write_begin = quote! {
        let _ = &self.#var_name.begin()?
    };

    let write_flush = Some(quote! {
        vec![self.#var_name.flush_async()?]
    });

    let write_shutdown = quote! {
        let _ = self.#var_name.shutdown()?
    };

    let read_constructor =
        quote! {
            #var_name: ReadOnlyIndexTable::new(
                storage.index_dbs.get(#var_name_literal).cloned().ok_or_else(|| TableError::TableDoesNotExist(format!("Index table '{}' not found", #var_name_literal)))?,
                #pk_by_index_table_name,
                #index_by_pk_table_name,
            )?
        };

    TxContextItem { write_definition, write_constructor, write_begin, write_flush, write_shutdown, read_definition, read_constructor }
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

    let write_constructor =
        quote! {
            #var_name: TableWriter::new(
                storage.index_dbs.get(#var_name_literal).cloned().ok_or_else(|| TableError::TableDoesNotExist(format!("Dict table '{}' not found", #var_name_literal)))?,
                DictFactory::new(
                    #dict_index_table_name,
                    #value_by_dict_pk_table_name,
                    #value_to_dict_pk_table_name,
                    #dict_pk_by_pk_table_name
                ),
            )?
        };

    let write_begin = quote! {
        let _ = &self.#var_name.begin()?
    };

    let write_flush = Some(quote! {
        vec![self.#var_name.flush_async()?]
    });

    let write_shutdown = quote! {
        let _ = self.#var_name.shutdown()?
    };

    let read_constructor =
        quote! {
            #var_name: ReadOnlyDictTable::new(
                storage.index_dbs.get(#var_name_literal).cloned().ok_or_else(|| TableError::TableDoesNotExist(format!("Dict table '{}' not found", #var_name_literal)))?,
                #dict_index_table_name,
                #value_by_dict_pk_table_name,
                #value_to_dict_pk_table_name,
                #dict_pk_by_pk_table_name
            )?
        };

    TxContextItem { write_definition, write_constructor, write_begin, write_flush, write_shutdown, read_definition, read_constructor }
}

pub fn tx_context_items(table_defs: &[TableDef]) -> Vec<TxContextItem> {
    table_defs
        .iter()
        .map(|def| tx_context_plain_item(def))
        .collect()
}

pub fn begin_write_fn_def(tx_context_ty: &Type) -> FunctionDef {
    let fn_name = format_ident!("begin_write_ctx");
    let fn_stream = quote! {
        pub fn #fn_name(storage: &Arc<Storage>) -> Result<#tx_context_ty, AppError> {
            #tx_context_ty::#fn_name(&storage)
        }
    };

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream: None,
        bench_stream: None,
    }

}

pub fn new_write_fn_def(tx_context_ty: &Type) -> FunctionDef {
    let fn_name = format_ident!("new_write_ctx");
    let fn_stream = quote! {
        pub fn #fn_name(storage: &Arc<Storage>) -> Result<#tx_context_ty, AppError> {
            #tx_context_ty::#fn_name(&storage)
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
    let fn_name = format_ident!("begin_read_ctx");
    let fn_stream = quote! {
        pub fn #fn_name(storage: &Arc<Storage>) -> Result<#tx_context_ty, AppError> {
            #tx_context_ty::begin_read_ctx(&storage)
        }
    };

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream: None,
        bench_stream: None,
    }

}