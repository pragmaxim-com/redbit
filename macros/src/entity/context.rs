use crate::rest::FunctionDef;
use crate::table::{TableDef, TableType};
use proc_macro2::{Ident, TokenStream};
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
    pub read_definition: TokenStream,
    pub read_init: TokenStream,
}

pub fn write_tx_context(entity_tx_context_ty: &Type, tx_contexts: &[TxContextItem]) -> TokenStream {
    let definitions: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_definition.clone()).collect();
    let inits: Vec<TokenStream> = tx_contexts.iter().map(|item| item.write_init.clone()).collect();
    let tx_context_name = tx_context_name(TxType::Write);
    quote! {
        pub struct #entity_tx_context_ty<'txn> {
            #(#definitions),*
        }
        impl<'txn> #tx_context_name<'txn> for #entity_tx_context_ty<'txn> {
           fn begin_write_tx(tx: &'txn WriteTransaction) -> Result<Self, TableError> {
                Ok(Self {
                    #(#inits),*
                })
            }
        }
    }
}
pub fn read_tx_context(entity_tx_context_ty: &Type, tx_contexts: &[TxContextItem]) -> TokenStream {
    let definitions: Vec<TokenStream> = tx_contexts.iter().map(|item| item.read_definition.clone()).collect();
    let inits: Vec<TokenStream> = tx_contexts.iter().map(|item| item.read_init.clone()).collect();
    let tx_context_name = tx_context_name(TxType::Read);
    quote! {
        pub struct #entity_tx_context_ty {
            #(#definitions),*
        }
        impl #tx_context_name for #entity_tx_context_ty {
           fn begin_read_tx(tx: &ReadTransaction) -> Result<Self, TableError> {
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
    let open_method = match def.table_type {
        TableType::DictIndex | TableType::Index => quote!(open_multimap_table),
        _ => quote!(open_table),
    };

    let write_table_type = match def.table_type {
        TableType::DictIndex | TableType::Index => quote!(MultimapTable),
        _ => quote!(Table),
    };

    let read_table_type = match def.table_type {
        TableType::DictIndex | TableType::Index => quote!(ReadOnlyMultimapTable),
        _ => quote!(ReadOnlyTable),
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
            #field_name: tx.#open_method(#table_name)?
        };

    TxContextItem { write_definition, write_init: init.clone(), read_definition, read_init: init }
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
        pub fn #fn_name<'txn>(write_tx: &'txn WriteTransaction) -> Result<#tx_context_ty<'txn>, TableError> {
            #tx_context_ty::begin_write_tx(write_tx)
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
        pub fn #fn_name(read_tx: &ReadTransaction) -> Result<#tx_context_ty, TableError> {
            #tx_context_ty::begin_read_tx(read_tx)
        }
    };

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream: None,
        bench_stream: None,
    }

}