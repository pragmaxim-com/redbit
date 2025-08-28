use crate::rest::FunctionDef;
use crate::table::{TableDef, TableType};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Type;

static TX_CONTEXT: &str = "TxContext";

pub fn tx_context_type(entity_type: &Type) -> Type {
    let suffix = TX_CONTEXT.to_string();
    let entity_ident = match entity_type {
        Type::Path(p) => p.path.segments.last().unwrap().ident.clone(),
        _ => panic!("Unsupported entity type for tx context"),
    };
    let tx_context_type = format_ident!("{}{}", entity_ident, suffix);
    syn::parse_quote!(#tx_context_type)
}


#[derive(Clone)]
pub struct TxContextItem {
    pub definition: TokenStream,
    pub init: TokenStream,
}

pub fn write_tx_context(tx_context_ty: &Type, tx_contexts: &[TxContextItem]) -> TokenStream {
    let definitions: Vec<TokenStream> = tx_contexts.iter().map(|item| item.definition.clone()).collect();
    let inits: Vec<TokenStream> = tx_contexts.iter().map(|item| item.init.clone()).collect();
    quote! {
        pub struct #tx_context_ty<'txn> {
            #(#definitions),*
        }
        impl<'txn> TxContext<'txn> for #tx_context_ty<'txn> {
           fn begin_write_tx(tx: &'txn WriteTransaction) -> Result<Self, TableError> {
                Ok(Self {
                    #(#inits),*
                })
            }
        }
    }
}

pub fn write_tx_context_item(def: &TableDef) -> TxContextItem {
    let field_name = format_ident!("{}", def.name.to_string().to_lowercase());
    let key_type = &def.key_type;
    let value_type = def.value_type.clone().unwrap_or_else(|| syn::parse_str::<Type>("()").unwrap());
    let table_name = &def.name;
    let open_method = match def.table_type {
        TableType::DictIndex | TableType::Index => quote!(open_multimap_table),
        _ => quote!(open_table),
    };

    let table_type = match def.table_type {
        TableType::DictIndex | TableType::Index => quote!(MultimapTable),
        _ => quote!(Table),
    };

    let definition =
        quote! {
            pub #field_name: #table_type<'txn, #key_type, #value_type>
        };

    let init =
        quote! {
            #field_name: tx.#open_method(#table_name)?
        };

    TxContextItem { definition, init }
}

pub fn write_tx_context_items(table_defs: &[TableDef]) -> Vec<TxContextItem> {
    table_defs
        .iter()
        .map(|def| write_tx_context_item(def))
        .collect()
}

pub fn begin_write_fn_def(tx_context_ty: &Type) -> FunctionDef {
    let fn_name = format_ident!("begin_write_tx");
    let fn_stream = quote! {
        pub fn #fn_name<'txn>(write_tx: &'txn WriteTransaction) -> Result<#tx_context_ty<'txn>, TableError> {
            #tx_context_ty::begin_write_tx(&write_tx)
        }
    };

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream: None,
        bench_stream: None,
    }

}