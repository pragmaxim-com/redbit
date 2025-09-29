use crate::rest::FunctionDef;
use crate::table::{DictTableDefs, IndexTableDefs, PlainTableDef};
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

pub fn tx_context_plain_item(def: &PlainTableDef) -> TxContextItem {
    let var_ident   = &def.var_name;
    let name_lit    = Literal::string(&var_ident.to_string());
    let key_ty      = &def.key_type;
    let val_ty: Type = def.value_type.clone().unwrap_or_else(|| syn::parse_str::<Type>("()").unwrap());
    let table_name  = &def.underlying.name;
    let shards      = def.column_props.shards;
    let is_sharded  = shards >= 2;

    let write_definition = if is_sharded {
        quote! {
            pub #var_ident: ShardedTableWriter<#key_ty, #val_ty, PlainFactory<#key_ty, #val_ty>, BytesPartitioner, Xxh3Partitioner>
        }
    } else {
        quote! {
            pub #var_ident: TableWriter<#key_ty, #val_ty, PlainFactory<#key_ty, #val_ty>>
        }
    };

    let read_definition = if is_sharded {
        quote! {
            pub #var_ident: ShardedReadOnlyPlainTable<#key_ty, #val_ty, BytesPartitioner>
        }
    } else {
        quote! {
            pub #var_ident: ReadOnlyPlainTable<#key_ty, #val_ty>
        }
    };

    // --- constructors (plain vs sharded) ---
    let write_constructor = if is_sharded {
        quote! {
            #var_ident: ShardedTableWriter::new(
                Partitioning::by_key(#shards),
                storage.fetch_sharded_dbs(#name_lit, Some(#shards))?,
                PlainFactory::new(#table_name),
            )?
        }
    } else {
        quote! {
            #var_ident: TableWriter::new(
                storage.fetch_single_db(#name_lit)?,
                PlainFactory::new(#table_name),
            )?
        }
    };

    let read_constructor = if is_sharded {
        quote! {
            #var_ident: ShardedReadOnlyPlainTable::new(
                BytesPartitioner::new(#shards),
                storage.fetch_sharded_dbs(#name_lit, Some(#shards))?,
                #table_name
            )?
        }
    } else {
        quote! {
            #var_ident: ReadOnlyPlainTable::new(
                storage.fetch_single_db(#name_lit)?,
                #table_name
            )?
        }
    };

    // --- common ops (identical for both variants) ---
    let write_begin = quote! { let _ = &self.#var_ident.begin()? };
    let write_flush = Some(quote! { self.#var_ident.flush_async()? });
    let write_shutdown = quote! { self.#var_ident.shutdown()? };

    TxContextItem {
        write_definition,
        write_constructor,
        write_begin,
        write_flush,
        write_shutdown,
        read_definition,
        read_constructor,
    }
}

pub fn tx_context_index_item(defs: &IndexTableDefs) -> TxContextItem {
    let var_ident    = &defs.var_name;
    let name_lit     = Literal::string(&var_ident.to_string());
    let key_ty       = &defs.key_type;
    let val_ty       = &defs.value_type;
    let pk_by_index  = &defs.pk_by_index.name;
    let index_by_pk  = &defs.index_by_pk.name;
    let lru_cache    = defs.column_props.lru_cache_size;
    let shards       = defs.column_props.shards;          // compile-time choice
    let is_sharded   = shards >= 2;

    // --- type definitions (plain vs sharded) ---
    let write_definition = if is_sharded {
        quote! {
            pub #var_ident: ShardedTableWriter<#key_ty, #val_ty, IndexFactory<#key_ty, #val_ty>, BytesPartitioner, Xxh3Partitioner>
        }
    } else {
        quote! {
            pub #var_ident: TableWriter<#key_ty, #val_ty, IndexFactory<#key_ty, #val_ty>>
        }
    };

    let read_definition = if is_sharded {
        quote! {
            pub #var_ident: ShardedReadOnlyIndexTable<#key_ty, #val_ty, Xxh3Partitioner>
        }
    } else {
        quote! {
            pub #var_ident: ReadOnlyIndexTable<#key_ty, #val_ty>
        }
    };

    // --- constructors (plain vs sharded) ---
    let write_constructor = if is_sharded {
        quote! {
            #var_ident: ShardedTableWriter::new(
                Partitioning::by_value(#shards),
                storage.fetch_sharded_dbs(#name_lit, Some(#shards))?,
                IndexFactory::new(#lru_cache, #pk_by_index, #index_by_pk),
            )?
        }
    } else {
        quote! {
            #var_ident: TableWriter::new(
                storage.fetch_single_db(#name_lit)?,
                IndexFactory::new(#lru_cache, #pk_by_index, #index_by_pk),
            )?
        }
    };

    let read_constructor = if is_sharded {
        quote! {
            #var_ident: ShardedReadOnlyIndexTable::new(
                Xxh3Partitioner::new(#shards),
                storage.fetch_sharded_dbs(#name_lit, Some(#shards))?,
                #pk_by_index,
                #index_by_pk,
            )?
        }
    } else {
        quote! {
            #var_ident: ReadOnlyIndexTable::new(
                storage.fetch_single_db(#name_lit)?,
                #pk_by_index,
                #index_by_pk,
            )?
        }
    };

    // --- common ops ---
    let write_begin    = quote! { let _ = &self.#var_ident.begin()? };
    let write_flush    = Some(quote! { self.#var_ident.flush_async()? });
    let write_shutdown = quote! { self.#var_ident.shutdown()? };

    TxContextItem {
        write_definition,
        write_constructor,
        write_begin,
        write_flush,
        write_shutdown,
        read_definition,
        read_constructor,
    }
}

pub fn tx_context_dict_item(defs: &DictTableDefs) -> TxContextItem {
    let var_ident   = &defs.var_name;
    let name_lit    = Literal::string(&var_ident.to_string());
    let key_ty      = &defs.key_type;
    let val_ty      = &defs.value_type;

    let dict_pk_to_ids = &defs.dict_pk_to_ids_table_def.name;
    let value_by_dict  = &defs.value_by_dict_pk_table_def.name;
    let value_to_dict  = &defs.value_to_dict_pk_table_def.name;
    let dict_pk_by_pk  = &defs.dict_pk_by_pk_table_def.name;

    // keep a sensible floor on LRU size
    let lru_cache = core::cmp::max(defs.column_props.lru_cache_size, 20_000);

    let shards     = defs.column_props.shards;
    let is_sharded = shards >= 2;

    // --- type definitions (plain vs sharded) ---
    let write_definition = if is_sharded {
        quote! {
            pub #var_ident: ShardedTableWriter<#key_ty, #val_ty, DictFactory<#key_ty, #val_ty>, BytesPartitioner, Xxh3Partitioner>
        }
    } else {
        quote! {
            pub #var_ident: TableWriter<#key_ty, #val_ty, DictFactory<#key_ty, #val_ty>>
        }
    };

    let read_definition = if is_sharded {
        quote! {
            pub #var_ident: ShardedReadOnlyDictTable<#key_ty, #val_ty, Xxh3Partitioner>
        }
    } else {
        quote! {
            pub #var_ident: ReadOnlyDictTable<#key_ty, #val_ty>
        }
    };

    // --- constructors (plain vs sharded) ---
    let write_constructor = if is_sharded {
        quote! {
            #var_ident: ShardedTableWriter::new(
                Partitioning::by_value(#shards),
                storage.fetch_sharded_dbs(#name_lit, Some(#shards))?,
                DictFactory::new(#lru_cache, #dict_pk_to_ids, #value_by_dict, #value_to_dict, #dict_pk_by_pk),
            )?
        }
    } else {
        quote! {
            #var_ident: TableWriter::new(
                storage.fetch_single_db(#name_lit)?,
                DictFactory::new(#lru_cache, #dict_pk_to_ids, #value_by_dict, #value_to_dict, #dict_pk_by_pk),
            )?
        }
    };

    let read_constructor = if is_sharded {
        quote! {
            #var_ident: ShardedReadOnlyDictTable::new(
                Xxh3Partitioner::new(#shards),
                storage.fetch_sharded_dbs(#name_lit, Some(#shards))?,
                #dict_pk_to_ids,
                #value_by_dict,
                #value_to_dict,
                #dict_pk_by_pk
            )?
        }
    } else {
        quote! {
            #var_ident: ReadOnlyDictTable::new(
                storage.fetch_single_db(#name_lit)?,
                #dict_pk_to_ids,
                #value_by_dict,
                #value_to_dict,
                #dict_pk_by_pk
            )?
        }
    };

    // --- common ops ---
    let write_begin    = quote! { let _ = &self.#var_ident.begin()? };
    let write_flush    = Some(quote! { self.#var_ident.flush_async()? });
    let write_shutdown = quote! { self.#var_ident.shutdown()? };

    TxContextItem {
        write_definition,
        write_constructor,
        write_begin,
        write_flush,
        write_shutdown,
        read_definition,
        read_constructor,
    }
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