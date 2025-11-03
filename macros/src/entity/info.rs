use crate::endpoint::EndpointDef;
use crate::field_parser::EntityDef;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_quote, Type};

static TABLE_INFO: &str = "TableInfo";

pub fn table_info_type(entity_type: &Type) -> Type {
    let suffix = TABLE_INFO.to_string();
    let entity_ident = match entity_type {
        Type::Path(p) => p.path.segments.last().unwrap().ident.clone(),
        _ => panic!("Unsupported entity type for stream query"),
    };
    let table_info_type = format_ident!("{}{}", entity_ident, suffix);
    syn::parse_quote!(#table_info_type)
}

pub fn table_info_fn(entity_def: &EntityDef) -> FunctionDef {
    let entity_name = &entity_def.entity_name;
    let table_info_type = &entity_def.info_type;
    let tx_context_ty = &entity_def.read_ctx_type;
    let fn_name = format_ident!("table_info");
    let fn_stream = quote! {
        pub fn #fn_name(storage: &Arc<Storage>) -> Result<#table_info_type, AppError> {
            #table_info_type::new_table_info(&#tx_context_ty::begin_read_ctx(&storage)?)
        }
    };

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: Some(parse_quote! { #table_info_type }),
            tag: EndpointTag::MetaRead,
            fn_name: fn_name.clone(),
            params: vec![],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<#table_info_type>, AppError> {
                    #entity_name::#fn_name(&state.storage).map(AppJson)
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/json", body = #table_info_type),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{}", entity_name.to_string().to_lowercase(), fn_name),
        }.to_endpoint()),
        test_stream: None,
        bench_stream: None
    }

}

#[derive(Clone)]
pub struct TableInfoItem {
    pub definition: TokenStream,
    pub init: TokenStream,
}

pub fn table_info_struct(entity_def: &EntityDef, table_info_items: &[TableInfoItem]) -> TokenStream {
    let table_info_ty = &entity_def.info_type;
    let definitions: Vec<TokenStream> = table_info_items.iter().map(|item| item.definition.clone()).collect();
    let inits: Vec<TokenStream> = table_info_items.iter().map(|item| item.init.clone()).collect();
    let read_tx_context_ty = &entity_def.read_ctx_type;
    quote! {
        #[derive(Clone, Debug, IntoParams, Serialize, Deserialize, Default, ToSchema)]
        pub struct #table_info_ty {
            #(#definitions),*
        }
        impl #table_info_ty {
            pub fn new_table_info(tx_context: &#read_tx_context_ty) -> Result<#table_info_ty, AppError> {
                Ok(
                    Self {
                        #(#inits),*
                    }
                )
            }
        }
    }
}
