use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use std::fmt::Display;
use syn::Type;

#[derive(Clone)]
pub struct HttpEndpointMacro {
    pub endpoint_def: EndpointDef,
    pub handler_fn_name: Ident,
    pub handler: TokenStream,
}

impl Display for HttpEndpointMacro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let method = self.endpoint_def.method.to_string().to_ascii_uppercase();
        let prefix = format!("{}:{}", method, self.endpoint_def.endpoint);
        let indentation = 50;
        let pad = if prefix.len() >= indentation {
            1 // fallback spacing if prefix is too long
        } else {
            indentation - prefix.len()
        };
        write!(f, "{}{:pad$}{}", prefix, "", self.handler_fn_name, pad = pad)
    }
}

#[derive(Clone)]
pub struct FunctionDef {
    pub entity_name: Ident,
    pub fn_name: Ident,
    pub return_type: Type,
    pub fn_stream: TokenStream,
    pub endpoint_def: Option<EndpointDef>,
}

#[derive(Clone)]
pub struct GetParam {
    pub name: Ident,
    pub ty: Type,
    pub description: String
}

#[derive(Clone)]
pub struct PostParam {
    pub name: Ident,
    pub ty: Type,
    pub content_type: String
}

#[derive(Clone)]
pub enum ParamExtraction {
    FromPath(Vec<GetParam>),
    FromQuery(Vec<GetParam>),
    FromBody(PostParam),
}

#[derive(Clone, Debug)]
pub enum HttpMethod {
    GET,
    POST,
    DELETE,
    HEAD
}

impl Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            HttpMethod::GET => write!(f, "get"),
            HttpMethod::POST => write!(f, "post"),
            HttpMethod::DELETE => write!(f, "delete"),
            HttpMethod::HEAD => write!(f, "head"),
        }
    }
}

#[derive(Clone)]
pub struct EndpointDef {
    pub param_extraction: ParamExtraction,
    pub endpoint: String,
    pub method: HttpMethod,
    pub fn_call: TokenStream,
}

pub fn to_http_endpoints(defs: Vec<FunctionDef>) -> (Vec<HttpEndpointMacro>, Vec<TokenStream>) {
    let endpoints: Vec<HttpEndpointMacro> =
        defs.iter()
            .filter_map(|fn_def| fn_def.endpoint_def.clone().map(|e| to_http_endpoint(fn_def, &e)))
            .collect();
    let route_chains =
        endpoints
            .iter()
            .map(|e| {
                let function_name = &e.handler_fn_name;
                quote! { .merge(redbit::utoipa_axum::router::OpenApiRouter::new().routes(redbit::utoipa_axum::routes!(#function_name))) }
            })
            .collect();
    (endpoints, route_chains)
}

pub fn to_http_endpoint(fn_def: &FunctionDef, endpoint_def: &EndpointDef) -> HttpEndpointMacro {
    let handler_fn_name = format_ident!("{}_{}", fn_def.entity_name.to_string().to_lowercase(), fn_def.fn_name);
    let return_type = &fn_def.return_type;
    let fn_call = endpoint_def.fn_call.clone();
    let param_binding = match endpoint_def.param_extraction.clone() {
        ParamExtraction::FromPath(params) => {
            match &params[..] {
                [] => quote! {},
                [GetParam { name, ty, description: _}] => {
                    quote! { axum::extract::Path(#name): axum::extract::Path<#ty> }
                }
                _ => {
                    let bindings: Vec<Ident> = params.iter().map(|p| p.name.clone()).collect();
                    let types: Vec<&Type> = params.iter().map(|p| &p.ty).collect();
                    quote! { axum::extract::Path((#(#bindings),*)): axum::extract::Path<(#(#types),*)> }
                }
            }
        }
        ParamExtraction::FromQuery(params) => {
            match &params[..] {
                [] => quote! {},
                [GetParam { name, ty, description: _}] => {
                    quote! { axum::extract::Query(#name): axum::extract::Query<#ty> }
                }
                _ => {
                    let bindings: Vec<Ident> = params.iter().map(|p| p.name.clone()).collect();
                    let types: Vec<&Type> = params.iter().map(|p| &p.ty).collect();
                    quote! { axum::extract::Query((#(#bindings),*)): axum::extract::Query<(#(#types),*)> }
                }
            }
        }
        ParamExtraction::FromBody(PostParam {name, ty, content_type: _}) => {
            quote! { AppJson(#name): AppJson<#ty> }
        }
    };
    let endpoint_name = fn_def.entity_name.to_string();
    let endpoint_ident = fn_def.entity_name.clone();
    let endpoint_path = endpoint_def.endpoint.clone();
    let method_ident = format_ident!("{}", endpoint_def.method.to_string());
    let db_call = match endpoint_def.method {
        HttpMethod::GET | HttpMethod::HEAD =>
            quote! {
                state.db.begin_read()
                    .map_err(AppError::from)
                    .and_then(|tx| #fn_call)
                    .map(AppJson)
            },
        HttpMethod::POST | HttpMethod::DELETE =>
            quote! {
                let db = state.db;
                let result = #fn_call?;
                Ok(AppJson(result))
            },
    };

    let responses = match endpoint_def.method {
        HttpMethod::GET => quote! { responses((status = OK, body = #endpoint_ident)) }, // GET can return Vec<T> better use #return_type
        HttpMethod::POST => quote! { responses((status = OK, body = #return_type)) },
        HttpMethod::DELETE | HttpMethod::HEAD => quote! { responses((status = OK)) },
    };
    // params can be mapped also with IntoParams trait, but for now we use the explicit extraction

    let params = match endpoint_def.param_extraction.clone() {
        ParamExtraction::FromPath(params) => {
            let param_tokens: Vec<TokenStream> = params.iter().map(|param| {
                let name_str = Literal::string(&param.name.to_string());
                let ty = &param.ty;
                let desc = Literal::string(&param.description);
                quote! { (#name_str = #ty, Path, description = #desc) }
            }).collect();

            quote! {
                params( #(#param_tokens),* )
            }
        },
        ParamExtraction::FromQuery(params) => {
            let param_tokens: Vec<TokenStream> = params.iter().map(|param| {
                let name_str = Literal::string(&param.name.to_string());
                let ty = &param.ty;
                let desc = Literal::string(&param.description);
                quote! { (#name_str = #ty, Query, description = #desc) }
            }).collect();

            quote! {
                params( #(#param_tokens),* )
            }
        },
        ParamExtraction::FromBody(param) => {
            let content_type = Literal::string(&param.content_type);
            let param_type = param.ty;
            quote! { request_body(content = #param_type, content_type = #content_type) }
        },
    };

    let handler = quote! {
        #[redbit::utoipa::path(#method_ident, path = #endpoint_path, #params, #responses, tag = #endpoint_name)]
        #[axum::debug_handler]
        pub async fn #handler_fn_name(
            axum::extract::State(state): axum::extract::State<RequestState>, #param_binding
        ) -> Result<AppJson<#return_type>, AppError> {
            #db_call
        }
    };

    HttpEndpointMacro {
        endpoint_def: endpoint_def.clone(),
        handler_fn_name,
        handler,
    }
}