use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use std::fmt::Display;
use syn::Type;

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

impl HttpParams {
    pub fn axum_bindings(&self) -> TokenStream {
        match self {
            HttpParams::FromPath(params) => match &params[..] {
                [] => quote! {},
                [GetParam { name, ty, description: _ }] => {
                    quote! { axum::extract::Path(#name): axum::extract::Path<#ty> }
                }
                _ => {
                    let bindings: Vec<Ident> = params.iter().map(|p| p.name.clone()).collect();
                    let types: Vec<&Type> = params.iter().map(|p| &p.ty).collect();
                    quote! { axum::extract::Path((#(#bindings),*)): axum::extract::Path<(#(#types),*)> }
                }
            },
            HttpParams::FromQuery(ty) => {
                quote! { axum::extract::Query(query): axum::extract::Query<#ty> }
            }
            HttpParams::FromBody(PostParam { name, ty, content_type: _ }) => {
                quote! { AppJson(#name): AppJson<#ty> }
            }
        }
    }

    pub fn utoipa_params(&self) -> TokenStream {
        match self {
            HttpParams::FromPath(params) => {
                let param_tokens: Vec<TokenStream> = params
                    .iter()
                    .map(|param| {
                        let name_str = Literal::string(&param.name.to_string());
                        let ty = &param.ty;
                        let desc = Literal::string(&param.description);
                        quote! { (#name_str = #ty, Path, description = #desc) }
                    })
                    .collect();

                quote! {
                    params( #(#param_tokens),* )
                }
            }
            HttpParams::FromQuery(ty) => {
                quote! { params(#ty) }
            }
            HttpParams::FromBody(param) => {
                let content_type = Literal::string(&param.content_type);
                let param_type = param.ty.clone();
                quote! { request_body(content = #param_type, content_type = #content_type) }
            }
        }
    }
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

#[derive(Clone)]
pub struct EndpointDef {
    pub params: HttpParams,
    pub endpoint: String,
    pub method: HttpMethod,
    pub return_type: Option<Type>,
}

impl EndpointDef {
    pub fn generate_test(&self) -> TokenStream {
        let method_name = match self.method {
            HttpMethod::GET => quote! { http::Method::GET },
            HttpMethod::POST => quote! { http::Method::POST },
            HttpMethod::HEAD => quote! { http::Method::HEAD },
            HttpMethod::DELETE => quote! { http::Method::DELETE },
        };

        fn generate_path_expr(endpoint_path: &str, params: &[GetParam]) -> TokenStream {
            let mut fmt_string = endpoint_path.to_string();
            let mut fmt_args = Vec::new();

            for GetParam { name, ty, .. } in params {
                let placeholder = format!("{{{}}}", name);
                if fmt_string.contains(&placeholder) {
                    fmt_string = fmt_string.replace(&placeholder, "{}");
                    fmt_args.push(quote! { <#ty as Default>::default().encode() });
                }
            }

            if fmt_args.is_empty() {
                quote! {
                    #fmt_string.to_string()
                }
            } else {
                quote! {
                    format!(#fmt_string, #(#fmt_args),*)
                }
            }
        }

        match &self.params {
            HttpParams::FromPath(params) => {
                let path_expr = generate_path_expr(&self.endpoint, params);
                quote! {
                    {
                        let endpoint_path = #path_expr;
                        eprintln!("Testing endpoint: {} : {}", #method_name, endpoint_path);
                        let response = server.method(#method_name, &endpoint_path).await;
                        response.assert_status_ok();
                    }
                }
            }

            HttpParams::FromQuery(ty) => {
                let endpoint_path = &self.endpoint;
                quote! {
                    {
                        for sample in #ty::sample() {
                            let query_string = serde_urlencoded::to_string(&sample).unwrap();
                            let endpoint_path = format!("{}?{}", #endpoint_path, query_string);
                            eprintln!("Testing endpoint: {} : {}", #method_name, #endpoint_path);
                            let response = server.method(#method_name, &endpoint_path).await;
                            response.assert_status_ok();
                        }
                    }
                }
            }

            HttpParams::FromBody(param) => {
                let endpoint_path = &self.endpoint;
                let param_type = &param.ty;
                let default_value = quote! { #param_type::sample() };
                quote! {
                    {
                        let response = server.method(#method_name, #endpoint_path).json(&#default_value).await;
                        eprintln!("Testing endpoint: {} : {}", #method_name, #endpoint_path);
                        response.assert_status_ok();
                    }
                }
            }
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
            quote! { .merge(redbit::utoipa_axum::router::OpenApiRouter::new().routes(redbit::utoipa_axum::routes!(#function_name))) }
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
    let param_binding = endpoint_def.params.axum_bindings();
    let endpoint_name = fn_def.entity_name.to_string();
    let endpoint_path = endpoint_def.endpoint.clone();
    let method_ident = format_ident!("{}", endpoint_def.method.to_string());
    let handler_method_def = match endpoint_def.method {
        HttpMethod::HEAD | HttpMethod::GET if !fn_def.is_sse => quote! {
            pub async fn #handler_fn_name(
                axum::extract::State(state): axum::extract::State<RequestState>, 
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
                axum::extract::State(state): axum::extract::State<RequestState>,
                #param_binding
            ) -> impl axum::response::IntoResponse {
               use axum::response::IntoResponse;
               match state.db.begin_read()
                    .map_err(AppError::from)
                    .and_then(|tx| #fn_call) {
                        Ok(stream) => redbit::axum_streams::StreamBodyAs::json_nl_with_errors(stream).into_response(),
                        Err(err)   => err.into_response(),
                }
            }
        },
        HttpMethod::POST | HttpMethod::DELETE if !fn_def.is_sse => quote! {
            pub async fn #handler_fn_name(
                axum::extract::State(state): axum::extract::State<RequestState>,
                #param_binding
            ) -> Result<AppJson<#fn_return_type>, AppError> {
                let db = state.db;
                let result = #fn_call?;
                Ok(AppJson(result))
            }
        },
        _ => quote! {
            redbit::AppError::from(format!("Unsupported HTTP method: {}", endpoint_def.method)).to_compile_error().into();
        }
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

    let params = endpoint_def.params.utoipa_params();

    let handler = quote! {
        #[redbit::utoipa::path(#method_ident, path = #endpoint_path, #params, #utoipa_response, tag = #endpoint_name)]
        #[axum::debug_handler]
        #handler_method_def
    };

    HttpEndpointMacro { endpoint_def: endpoint_def.clone(), handler_fn_name, handler, test: endpoint_def.generate_test() }
}
