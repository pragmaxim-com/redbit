use proc_macro2::{Ident, TokenStream};
use syn::Type;
use quote::{format_ident, quote};

#[derive(Clone)]
pub struct HttpEndpointMacro {
    pub endpoint: String,
    pub fn_name: Ident,
    pub handler: TokenStream,
}

#[derive(Clone)]
pub struct FunctionDef {
    pub entity: Ident,
    pub name: Ident,
    pub stream: TokenStream,
    pub return_value: ReturnValue,
    pub endpoint: Option<Endpoint>
}

#[derive(Clone)]
pub struct ReturnValue {
    pub value_name: Ident,
    pub value_type: Type,
}

#[derive(Clone)]
pub enum Endpoint {
    GetBy(Params),    // `/entity/column/:column_name`
    RangeBy(Params),  // `/entity/column?from=&to=`
    Relation(Params), // `/entity/:pk/relation`
    Take,   // `/entity?take=1000`
    First,   // `/entity?first=true`
    Last,   // `/entity?last=true`
}

#[derive(Clone)]
pub struct Params { // currently only params of one type are supported
    pub column_name: Ident,
    pub column_type: Type,
}

pub fn to_http_endpoint(def: &FunctionDef) -> Option<HttpEndpointMacro> {
    if let Some(endpoint_type) = &def.endpoint {
        let entity_snake = def.entity.to_string().to_lowercase();
        let method = &def.name;
        let entity = &def.entity;
        let return_type = &def.return_value.value_type;

        let (endpoint, fn_name, param_binding, db_call) = match endpoint_type {
            Endpoint::GetBy(p) => {
                let column = p.column_name.clone();
                let column_type = p.column_type.clone();
                let fn_name = format_ident!("{}_handle_by_{}", entity_snake, column);
                let path = format!("/{}/{}/{{value}}", entity_snake, column);
                let param_type = quote! { RequestByParams<#column_type> };
                let extract = quote! { ::axum::extract::Path(params): ::axum::extract::Path<#param_type> };
                let db = quote! { #entity::#method(&read_tx, &params.value) };
                (path, fn_name, extract, db)
            }
            Endpoint::RangeBy(p) => {
                let column = p.column_name.clone();
                let column_type = p.column_type.clone();
                let fn_name = format_ident!("{}_handle_range_by_{}", entity_snake, column);
                let path = format!("/{}/{}?from=&until=", entity_snake, column);
                let param_type = quote! { RequestRangeParams<#column_type, #column_type> };
                let extract = quote! { ::axum::extract::Query(params): ::axum::extract::Query<#param_type> };
                let db = quote! { #entity::#method(&read_tx, &params.from, &params.until) };
                (path, fn_name, extract, db)
            }
            Endpoint::Relation(p) => {
                let column_type = p.column_type.clone();
                let relation_entity_name = &def.return_value.value_name;
                let fn_name = format_ident!("{}_handle_relation_{}", entity_snake, relation_entity_name);
                let path = format!("/{}/{{value}}/{}", entity_snake, relation_entity_name);
                let param_type = quote! { RequestByParams<#column_type> };
                let extract = quote! { ::axum::extract::Path(params): ::axum::extract::Path<#param_type> };
                let db = quote! { #entity::#method(&read_tx, &params.value) };
                (path, fn_name, extract, db)
            }
            Endpoint::Take => {
                let fn_name = format_ident!("{}_handle_take", entity_snake);
                let path = format!("/{}?take=", entity_snake);
                let param_type = quote! { TakeParams };
                let extract = quote! { ::axum::extract::Query(params): ::axum::extract::Query<#param_type> };
                let db = quote! { #entity::#method(&read_tx, params.take) };
                (path, fn_name, extract, db)
            }
            Endpoint::First => {
                let fn_name = format_ident!("{}_handle_first", entity_snake);
                let path = format!("/{}?first=", entity_snake);
                let param_type = quote! { FirstParams };
                let extract = quote! { ::axum::extract::Query(params): ::axum::extract::Query<#param_type> };
                let db = quote! { #entity::#method(&read_tx) };
                (path, fn_name, extract, db)
            }
            Endpoint::Last => {
                let fn_name = format_ident!("{}_handle_last", entity_snake);
                let path = format!("/{}?last=", entity_snake);
                let param_type = quote! { LastParams };
                let extract = quote! { ::axum::extract::Query(params): ::axum::extract::Query<#param_type> };
                let db = quote! { #entity::#method(&read_tx) };
                (path, fn_name, extract, db)
            }
        };

        let handler = quote! {
        #[axum::debug_handler]
        pub async fn #fn_name(
            ::axum::extract::State(state): ::axum::extract::State<RequestState>,
            #param_binding,
        ) -> Result<AppJson<#return_type>, AppError> {
            state.db.begin_read()
                .map_err(AppError::from)
                .and_then(|read_tx| #db_call)
                .map(AppJson)
        }
    };

        Some(HttpEndpointMacro {
            endpoint,
            fn_name,
            handler,
        })
    } else {
        None
    }
}