use crate::rest::FunctionDef;
use crate::table::{DictTableDefs, IndexTableDefs, PlainTableDef};
use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use syn::Type;
use crate::field_parser::EntityDef;

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

pub fn entity_tx_context_def_type(entity_type: &Type) -> Type {
    let entity_ident = match entity_type {
        Type::Path(p) => p.path.segments.last().unwrap().ident.clone(),
        _ => panic!("Unsupported entity type for tx context"),
    };
    let tx_context_type = format_ident!("{}{}", entity_ident, format_ident!("{}", TX_CONTEXT));
    syn::parse_quote!(#tx_context_type)
}

#[derive(Clone)]
pub struct TxContextItem {
    pub var_name: Ident,
    pub definition: TokenStream,
    pub def_constructor: TokenStream,
    pub write_definition: TokenStream,
    pub write_shutdown: TokenStream,
    pub read_definition: TokenStream,
}

pub fn def_tx_context(entity_def: &EntityDef, tx_contexts: &[TxContextItem]) -> TokenStream {
    let definitions: Vec<TokenStream> = tx_contexts.iter().map(|item| item.definition.clone()).collect();
    let constructors: Vec<TokenStream> = tx_contexts.iter().map(|item| item.def_constructor.clone()).collect();
    let entity_tx_context_ty = &entity_def.ctx_type;
    let read_entity_tx_context_ty = &entity_def.read_ctx_type;
    let write_entity_tx_context_ty = &entity_def.write_ctx_type;
    let tx_context_name = format_ident!("{}", TX_CONTEXT);
    quote! {
        #[derive(Debug)]
        pub struct #entity_tx_context_ty {
            #(#definitions),*
        }
        impl #tx_context_name for #entity_tx_context_ty {
            type ReadCtx = #read_entity_tx_context_ty;
            type WriteCtx = #write_entity_tx_context_ty;
            fn definition() -> Result<Self, AppError> {
                Ok(Self {
                    #(#constructors),*
                })
            }
        }
    }
}

pub fn write_tx_context(entity_def: &EntityDef, tx_contexts: &[TxContextItem]) -> TokenStream {
    let definitions: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_definition.clone()).collect();
    let constructors: Vec<TokenStream> =
        tx_contexts.iter()
            .map(|item| item.var_name.clone())
            .map(|var_name| quote!{#var_name: defs.#var_name.to_write_field(storage)?}).collect();
    let var_names: Vec<Ident> = tx_contexts.iter().map(|item| item.var_name.clone()).collect();
    let shutdowns: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_shutdown.clone()).collect();
    let write_tx_context_name = tx_context_name(TxType::Write);
    let write_tx_context_ty = &entity_def.write_ctx_type;
    let entity_tx_context_ty = &entity_def.ctx_type;
    let len = tx_contexts.len();
    quote! {
        pub struct #write_tx_context_ty {
            #(#definitions),*
        }
        impl #write_tx_context_name for #write_tx_context_ty {
           type Defs = #entity_tx_context_ty;
           type WriterRefs<'a> = [&'a dyn WriteComponentRef; #len];
           fn new_write_ctx(defs: &Self::Defs, storage: &Arc<Storage>) -> redb::Result<Self, AppError> {
                Ok(Self {
                    #(#constructors),*
                })
            }
            fn writer_refs(&self) -> Self::WriterRefs<'_> {
                [ #( &self.#var_names ),* ]
            }
            fn stop_writing_async(self) -> Result<Vec<StopFuture>, AppError> {
                let mut futures: Vec<StopFuture> = Vec::new();
                #( futures.extend(#shutdowns); )*
                Ok(futures)
            }
        }
    }
}

pub fn read_tx_context(entity_def: &EntityDef, tx_contexts: &[TxContextItem]) -> TokenStream {
    let definitions: Vec<TokenStream> = tx_contexts.iter().map(|item| item.read_definition.clone()).collect();
    let constructors: Vec<TokenStream> =
        tx_contexts.iter()
            .map(|item| item.var_name.clone())
            .map(|var_name| quote!{#var_name: defs.#var_name.to_read_field(storage)?}).collect();
    let entity_tx_context_ty = &entity_def.ctx_type;
    let read_tx_context_ty = &entity_def.read_ctx_type;
    let read_tx_context_name = tx_context_name(TxType::Read);
    quote! {
        pub struct #read_tx_context_ty {
            #(#definitions),*
        }
        impl #read_tx_context_name for #read_tx_context_ty {
            type Defs = #entity_tx_context_ty;
            fn begin_read_ctx(defs: &Self::Defs, storage: &Arc<Storage>) -> Result<Self, AppError> {
                Ok(Self {
                    #(#constructors),*
                })
            }
        }
    }
}

pub fn tx_context(entity_def: &EntityDef, tx_contexts: &[TxContextItem]) -> TokenStream {
    let write_ctx = write_tx_context(&entity_def, tx_contexts);
    let read_ctx = read_tx_context(&entity_def, tx_contexts);
    let ctx = def_tx_context(&entity_def, tx_contexts);
    quote! {
        #ctx
        #write_ctx
        #read_ctx
    }
}

pub fn tx_context_plain_item(def: &PlainTableDef) -> TxContextItem {
    let var_ident   = &def.var_name;
    let name_lit    = Literal::string(&var_ident.to_string());
    let key_ty      = &def.key_type;
    let val_ty: Type = def.value_type.clone().unwrap_or_else(|| syn::parse_str::<Type>("()").unwrap());
    let table_def = &def.underlying.definition;
    let shards= def.column_props.shards;
    let root_pk= def.root_pk;

    let definition =
        quote! {
            pub #var_ident: RedbitTableDefinition<#key_ty, #val_ty, BytesPartitioner, Xxh3Partitioner, PlainFactory<#key_ty, #val_ty>>
        };

    let write_definition =
        quote! {
            pub #var_ident: ShardedTableWriter<#key_ty, #val_ty, BytesPartitioner, Xxh3Partitioner, PlainFactory<#key_ty, #val_ty>>
        };

    let read_definition =
        quote! {
            pub #var_ident: ShardedTableReader<#key_ty, #val_ty, BytesPartitioner, Xxh3Partitioner>
        };

    let def_constructor = quote! {
        #var_ident: RedbitTableDefinition::new(
            #root_pk,
            Partitioning::by_key(#shards),
            PlainFactory::new(#name_lit, #table_def),
        )
    };
    let write_shutdown = quote! { self.#var_ident.shutdown_async()? };

    TxContextItem {
        var_name: var_ident.clone(),
        definition,
        def_constructor,
        write_definition,
        write_shutdown,
        read_definition,
    }
}

pub fn tx_context_index_item(defs: &IndexTableDefs) -> TxContextItem {
    let var_ident    = &defs.var_name;
    let name_lit     = Literal::string(&var_ident.to_string());
    let key_ty       = &defs.key_type;
    let val_ty       = &defs.value_type;
    let pk_by_index  = &defs.pk_by_index.definition;
    let index_by_pk  = &defs.index_by_pk.definition;
    let lru_cache    = defs.column_props.lru_cache_size;
    let shards       = defs.column_props.shards;          // compile-time choice

    let definition =
        quote! {
            pub #var_ident: RedbitTableDefinition<#key_ty, #val_ty, BytesPartitioner, Xxh3Partitioner, IndexFactory<#key_ty, #val_ty>>
        };

    let write_definition =
        quote! {
            pub #var_ident: ShardedTableWriter<#key_ty, #val_ty, BytesPartitioner, Xxh3Partitioner, IndexFactory<#key_ty, #val_ty>>
        };

    let read_definition =
        quote! {
            pub #var_ident: ShardedTableReader<#key_ty, #val_ty, BytesPartitioner, Xxh3Partitioner>
        };

    let def_constructor = quote! {
        #var_ident: RedbitTableDefinition::new(
            false,
            Partitioning::by_value(#shards),
            IndexFactory::new(#name_lit, #lru_cache, #pk_by_index, #index_by_pk),
        )
    };
    let write_shutdown = quote! { self.#var_ident.shutdown_async()? };

    TxContextItem {
        var_name: var_ident.clone(),
        definition,
        def_constructor,
        write_definition,
        write_shutdown,
        read_definition,
    }
}

pub fn tx_context_dict_item(defs: &DictTableDefs) -> TxContextItem {
    let var_ident   = &defs.var_name;
    let name_lit    = Literal::string(&var_ident.to_string());
    let key_ty      = &defs.key_type;
    let val_ty      = &defs.value_type;
    let lru_cache    = defs.column_props.lru_cache_size;
    let shards       = defs.column_props.shards;
    let dict_pk_to_ids = &defs.dict_pk_to_ids_table_def.definition;
    let value_by_dict  = &defs.value_by_dict_pk_table_def.definition;
    let value_to_dict  = &defs.value_to_dict_pk_table_def.definition;
    let dict_pk_by_pk  = &defs.dict_pk_by_pk_table_def.definition;

    let definition =
        quote! {
            pub #var_ident: RedbitTableDefinition<#key_ty, #val_ty, BytesPartitioner, Xxh3Partitioner, DictFactory<#key_ty, #val_ty>>
        };

    let write_definition =
        quote! {
            pub #var_ident: ShardedTableWriter<#key_ty, #val_ty, BytesPartitioner, Xxh3Partitioner, DictFactory<#key_ty, #val_ty>>
        };

    let read_definition =
        quote! {
            pub #var_ident: ShardedTableReader<#key_ty, #val_ty, BytesPartitioner, Xxh3Partitioner>
        };

    let def_constructor = quote! {
        #var_ident: RedbitTableDefinition::new(
            false,
            Partitioning::by_value(#shards),
            DictFactory::new(#name_lit, #lru_cache, #dict_pk_to_ids, #value_by_dict, #value_to_dict, #dict_pk_by_pk),
        )
    };
    let write_shutdown = quote! { self.#var_ident.shutdown_async()? };

    TxContextItem {
        var_name: var_ident.clone(),
        definition,
        def_constructor,
        write_definition,
        write_shutdown,
        read_definition,
    }
}

pub fn begin_write_fn_def(entity_def: &EntityDef) -> FunctionDef {
    let tx_context_ty = &entity_def.ctx_type;
    let write_tx_context_ty = &entity_def.write_ctx_type;
    let fn_name = format_ident!("begin_write_ctx");
    let fn_stream = quote! {
        pub fn #fn_name(storage: &Arc<Storage>, durability: Durability) -> Result<#write_tx_context_ty, AppError> {
            #tx_context_ty::definition()?.#fn_name(&storage, durability)
        }
    };

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream: None,
        bench_stream: None,
    }

}

pub fn definition(entity_def: &EntityDef) -> FunctionDef {
    let tx_context_ty = &entity_def.ctx_type;
    let fn_name = format_ident!("definition");
    let fn_stream = quote! {
        pub fn #fn_name() -> Result<#tx_context_ty, AppError> {
            #tx_context_ty::definition()
        }
    };

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream: None,
        bench_stream: None,
    }
}

pub fn new_write_fn_def(entity_def: &EntityDef) -> FunctionDef {
    let tx_context_ty = &entity_def.ctx_type;
    let write_tx_context_ty = &entity_def.write_ctx_type;
    let fn_name = format_ident!("new_write_ctx");
    let fn_stream = quote! {
        pub fn #fn_name(storage: &Arc<Storage>) -> Result<#write_tx_context_ty, AppError> {
            #tx_context_ty::definition()?.#fn_name(&storage)
        }
    };

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream: None,
        bench_stream: None,
    }
}

pub fn begin_read_fn_def(entity_def: &EntityDef) -> FunctionDef {
    let tx_context_ty = &entity_def.ctx_type;
    let read_tx_context_ty = &entity_def.read_ctx_type;
    let fn_name = format_ident!("begin_read_ctx");
    let fn_stream = quote! {
        pub fn #fn_name(storage: &Arc<Storage>) -> Result<#read_tx_context_ty, AppError> {
            #tx_context_ty::definition()?.#fn_name(&storage)
        }
    };

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream: None,
        bench_stream: None,
    }

}