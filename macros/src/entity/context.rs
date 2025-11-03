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
    pub async_flush: Option<TokenStream>,
    pub deferred_flush: Option<TokenStream>,
    pub write_shutdown: TokenStream,
    pub read_definition: TokenStream,
    pub read_constructor: TokenStream,
}

pub fn write_tx_context(entity_tx_context_ty: &Type, tx_contexts: &[TxContextItem]) -> TokenStream {
    let definitions: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_definition.clone()).collect();
    let constructors: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_constructor.clone()).collect();
    let begins: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_begin.clone()).collect();
    let shutdowns: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_shutdown.clone()).collect();
    let async_flushes: Vec<TokenStream> = tx_contexts.iter().flat_map(|item| item.async_flush.clone()).collect();
    let deferred_flushes: Vec<TokenStream> = tx_contexts.iter().flat_map(|item| item.deferred_flush.clone()).collect();
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
           fn begin_writing_async(&self, durability: Durability) -> Result<Vec<StartFuture>, AppError> {
                let mut futures: Vec<StartFuture> = Vec::new();
                #( futures.extend(#begins); )*
                Ok(futures)
            }
           fn stop_writing_async(self) -> Result<Vec<StopFuture>, AppError> {
                let mut futures: Vec<StopFuture> = Vec::new();
                #( futures.extend(#shutdowns); )*
                Ok(futures)
           }
           fn commit_ctx_async(&self) -> Result<Vec<FlushFuture>, AppError> {
                let mut futures: Vec<FlushFuture> = Vec::new();
                #( futures.extend(#async_flushes); )*
                Ok(futures)
           }
           fn commit_ctx_deferred(&self) -> Result<Vec<FlushFuture>, AppError> {
                let mut futures: Vec<FlushFuture> = Vec::new();
                #( futures.extend(#deferred_flushes); )*
                Ok(futures)
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

    let write_definition =
        quote! {
            pub #var_ident: ShardedTableWriter<#key_ty, #val_ty, PlainFactory<#key_ty, #val_ty>, BytesPartitioner, Xxh3Partitioner>
        };

    let read_definition =
        quote! {
            pub #var_ident: ShardedReadOnlyPlainTable<#key_ty, #val_ty, BytesPartitioner>
        };

    let write_constructor =
        quote! {
            #var_ident: ShardedTableWriter::new(
                Partitioning::by_key(#shards),
                storage.fetch_dbs(#name_lit, Some(#shards))?,
                PlainFactory::new(#name_lit, #table_name),
            )?
        };

    let read_constructor =
        quote! {
            #var_ident: ShardedReadOnlyPlainTable::new(
                BytesPartitioner::new(#shards),
                storage.fetch_dbs(#name_lit, Some(#shards))?,
                #table_name
            )?
        };

    let write_begin = quote! { self.#var_ident.begin_async(durability)? };
    let async_flush =
        if def.root_pk {
            Some(quote! { self.#var_ident.flush_two_phased()? })
        } else {
            Some(quote! { self.#var_ident.flush_async()? })
        };
    let deferred_flush = Some(quote! { self.#var_ident.flush_deferred()? });
    let write_shutdown = quote! { self.#var_ident.shutdown_async()? };

    TxContextItem {
        write_definition,
        write_constructor,
        write_begin,
        async_flush,
        deferred_flush,
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

    let write_definition =
        quote! {
            pub #var_ident: ShardedTableWriter<#key_ty, #val_ty, IndexFactory<#key_ty, #val_ty>, BytesPartitioner, Xxh3Partitioner>
        };

    let read_definition =
        quote! {
            pub #var_ident: ShardedReadOnlyIndexTable<#key_ty, #val_ty, Xxh3Partitioner>
        };

    let write_constructor = quote! {
        #var_ident: ShardedTableWriter::new(
            Partitioning::by_value(#shards),
            storage.fetch_dbs(#name_lit, Some(#shards))?,
            IndexFactory::new(#name_lit, #lru_cache, #pk_by_index, #index_by_pk),
        )?
    };

    let read_constructor =
        quote! {
            #var_ident: ShardedReadOnlyIndexTable::new(
                Xxh3Partitioner::new(#shards),
                storage.fetch_dbs(#name_lit, Some(#shards))?,
                #pk_by_index,
                #index_by_pk,
            )?
        };

    let write_begin    = quote! { self.#var_ident.begin_async(durability)? };
    let async_flush = Some(quote! { self.#var_ident.flush_async()? });
    let deferred_flush = Some(quote! { self.#var_ident.flush_deferred()? });
    let write_shutdown = quote! { self.#var_ident.shutdown_async()? };

    TxContextItem {
        write_definition,
        write_constructor,
        write_begin,
        async_flush,
        deferred_flush,
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
    let lru_cache    = defs.column_props.lru_cache_size;
    let shards     = defs.column_props.shards;
    let dict_pk_to_ids = &defs.dict_pk_to_ids_table_def.name;
    let value_by_dict  = &defs.value_by_dict_pk_table_def.name;
    let value_to_dict  = &defs.value_to_dict_pk_table_def.name;
    let dict_pk_by_pk  = &defs.dict_pk_by_pk_table_def.name;

    let write_definition = quote! {
            pub #var_ident: ShardedTableWriter<#key_ty, #val_ty, DictFactory<#key_ty, #val_ty>, BytesPartitioner, Xxh3Partitioner>
        };

    let read_definition =
        quote! {
            pub #var_ident: ShardedReadOnlyDictTable<#key_ty, #val_ty, Xxh3Partitioner>
        };

    let write_constructor = quote! {
        #var_ident: ShardedTableWriter::new(
            Partitioning::by_value(#shards),
            storage.fetch_dbs(#name_lit, Some(#shards))?,
            DictFactory::new(#name_lit, #lru_cache, #dict_pk_to_ids, #value_by_dict, #value_to_dict, #dict_pk_by_pk),
        )?
    };

    let read_constructor =
        quote! {
            #var_ident: ShardedReadOnlyDictTable::new(
                Xxh3Partitioner::new(#shards),
                storage.fetch_dbs(#name_lit, Some(#shards))?,
                #dict_pk_to_ids,
                #value_by_dict,
                #value_to_dict,
                #dict_pk_by_pk
            )?
        };

    let write_begin    = quote! { self.#var_ident.begin_async(durability)? };
    let async_flush = Some(quote! { self.#var_ident.flush_async()? });
    let deferred_flush = Some(quote! { self.#var_ident.flush_deferred()? });
    let write_shutdown = quote! { self.#var_ident.shutdown_async()? };

    TxContextItem {
        write_definition,
        write_constructor,
        write_begin,
        async_flush,
        deferred_flush,
        write_shutdown,
        read_definition,
        read_constructor,
    }
}

pub fn begin_write_fn_def(tx_context_ty: &Type) -> FunctionDef {
    let fn_name = format_ident!("begin_write_ctx");
    let fn_stream = quote! {
        pub fn #fn_name(storage: &Arc<Storage>, durability: Durability) -> Result<#tx_context_ty, AppError> {
            #tx_context_ty::#fn_name(&storage, durability)
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