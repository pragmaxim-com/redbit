use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::fmt::Display;
use syn::Type;
use crate::endpoint::EndpointDef;

#[derive(Clone)]
pub struct HttpEndpointMacro {
    pub endpoint_def: EndpointDef,
    pub handler_fn_name: Ident,
    pub handler: TokenStream,
    pub test: TokenStream,
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
    pub fn_return_type: Type,
    pub fn_stream: TokenStream,
    pub is_sse: bool,
    pub fn_call: TokenStream,
    pub endpoint_def: Option<EndpointDef>,
    pub test_stream: Option<TokenStream>,
}

#[derive(Clone)]
pub struct GetParam {
    pub name: Ident,
    pub ty: Type,
    pub description: String,
}

#[derive(Clone)]
pub struct PostParam {
    pub name: Ident,
    pub ty: Type,
    pub content_type: String,
}

#[derive(Clone)]
pub enum HttpParams {
    FromPath(Vec<GetParam>),
    FromQuery(Type),
    FromBody(PostParam),
}

#[derive(Clone)]
pub enum HttpMethod {
    GET,
    POST,
    DELETE,
    HEAD,
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

pub fn to_http_endpoints(defs: Vec<FunctionDef>) -> (Vec<TokenStream>, Vec<TokenStream>, Vec<TokenStream>) {
    let endpoints: Vec<HttpEndpointMacro> =
        defs.iter().filter_map(|fn_def| fn_def.endpoint_def.clone().map(|e| to_http_endpoint(fn_def, &e))).collect();
    let route_chains = endpoints
        .iter()
        .map(|e| {
            let function_name = &e.handler_fn_name;
            quote! { .merge(OpenApiRouter::new().routes(utoipa_axum::routes!(#function_name))) }
        })
        .collect();
    let endpoint_handlers: Vec<TokenStream> = endpoints.iter().map(|e| e.handler.clone()).collect();
    let tests: Vec<TokenStream> = endpoints.iter().map(|e| e.test.clone()).collect();
    (endpoint_handlers, route_chains, tests)
}

pub fn to_http_endpoint(fn_def: &FunctionDef, endpoint_def: &EndpointDef) -> HttpEndpointMacro {
    let handler_fn_name = format_ident!("{}_{}", fn_def.entity_name.to_string().to_lowercase(), fn_def.fn_name);
    let fn_return_type = &fn_def.fn_return_type;
    let fn_call = fn_def.fn_call.clone();
    let param_binding = endpoint_def.axum_bindings();
    let endpoint_name = fn_def.entity_name.to_string();
    let endpoint_path = endpoint_def.endpoint.clone();
    let method_ident = format_ident!("{}", endpoint_def.method.to_string());
    let handler_method_def = match endpoint_def.method {
        HttpMethod::HEAD | HttpMethod::GET if !fn_def.is_sse => quote! {
            pub async fn #handler_fn_name(
                extract::State(state): extract::State<RequestState>,
                #param_binding
            ) -> Result<AppJson<#fn_return_type>, AppError> {
                state.db.begin_read()
                    .map_err(AppError::from)
                    .and_then(|tx| #fn_call)
                    .map(AppJson)
            }
        },
        HttpMethod::GET if fn_def.is_sse => quote! {
            pub async fn #handler_fn_name(
                extract::State(state): extract::State<RequestState>,
                #param_binding
            ) -> impl axum::response::IntoResponse {
               match state.db.begin_read()
                    .map_err(AppError::from)
                    .and_then(|tx| #fn_call) {
                        Ok(stream) => axum_streams::StreamBodyAs::json_nl_with_errors(stream).into_response(),
                        Err(err)   => err.into_response(),
                }
            }
        },
        HttpMethod::POST | HttpMethod::DELETE if !fn_def.is_sse => quote! {
            pub async fn #handler_fn_name(
                extract::State(state): extract::State<RequestState>,
                #param_binding
            ) -> Result<AppJson<#fn_return_type>, AppError> {
                let db = state.db;
                let result = #fn_call?;
                Ok(AppJson(result))
            }
        },
        _ => quote! {
            redbit::AppError::from(format!("Unsupported HTTP method: {}", endpoint_def.method)).to_compile_error().into();
        },
    };

    let utoipa_response = match fn_def.is_sse {
        true => {
            let return_ty = endpoint_def.return_type.clone().expect("SSE endpoints must have a return type");
            quote! { responses((status = OK, content_type = "text/event-stream", body = #return_ty)) }
        }
        false => match endpoint_def.return_type.clone() {
            Some(return_ty) => quote! { responses((status = OK, body = #return_ty)) },
            None => quote! { responses((status = OK)) },
        },
    };

    let params = endpoint_def.utoipa_params();

    let handler = quote! {
        #[utoipa::path(#method_ident, path = #endpoint_path, #params, #utoipa_response, tag = #endpoint_name)]
        #[axum::debug_handler]
        #handler_method_def
    };

    HttpEndpointMacro { endpoint_def: endpoint_def.clone(), handler_fn_name, handler, test: endpoint_def.generate_test() }
}
