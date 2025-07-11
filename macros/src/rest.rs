use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use std::fmt::Display;
use syn::Type;
use crate::endpoint::EndpointDef;

#[derive(Clone)]
pub struct HttpEndpointMacro {
    pub endpoint_def: EndpointDef,
    pub handler_fn_name: Ident,
    pub handler: TokenStream,
    pub tests: Vec<TokenStream>,
    pub client_call: Option<String>,
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
    pub fn_stream: TokenStream,
    pub endpoint_def: Option<EndpointDef>,
    pub test_stream: Option<TokenStream>,
    pub bench_stream: Option<TokenStream>,
}

#[derive(Clone)]
pub struct PathExpr {
    pub name: Ident,
    pub ty: Type,
    pub sample: TokenStream,
    pub description: String,
}
#[derive(Clone)]
pub struct Param {
    pub name: Ident,
    pub ty: Type,
    pub samples: TokenStream,
    pub description: String,
}

#[derive(Clone)]
pub enum HttpParams {
    FromPath(Vec<PathExpr>),
    FromQuery(Param),
    FromBody(Param),
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

pub fn to_http_endpoints(defs: &Vec<FunctionDef>) -> (Vec<TokenStream>, TokenStream, Vec<TokenStream>, TokenStream) {
    let endpoints: Vec<HttpEndpointMacro> =
        defs.iter().filter_map(|fn_def| fn_def.endpoint_def.clone().map(|e| to_http_endpoint(fn_def, &e))).collect();
    let route_chains: Vec<TokenStream> = endpoints
        .iter()
        .map(|e| {
            let function_name = &e.handler_fn_name;
            quote! { .merge(OpenApiRouter::new().routes(utoipa_axum::routes!(#function_name))) }
        })
        .collect();
    let endpoint_handlers: Vec<TokenStream> = endpoints.iter().map(|e| e.handler.clone()).collect();
    let tests: Vec<TokenStream> = endpoints.iter().flat_map(|e| e.tests.clone()).collect();
    let client_calls: Vec<String> = endpoints.into_iter().filter_map(|e| e.client_call).collect();
    let client_calls_lit = Literal::string(&client_calls.join("\n"));
    let routes = quote! {
        pub fn routes() -> OpenApiRouter<RequestState> {
            OpenApiRouter::new()
                #(#route_chains)*
        }
    };
    let client_calls = quote! {
        pub fn client_calls() -> String {
            #client_calls_lit.to_string()
        }
    };
    (endpoint_handlers, routes, tests, client_calls)
}

pub fn to_http_endpoint(fn_def: &FunctionDef, endpoint_def: &EndpointDef) -> HttpEndpointMacro {
    let handler_fn_name = endpoint_def.handler_name.clone();
    let param_binding = endpoint_def.axum_bindings();
    let endpoint_name = fn_def.entity_name.to_string();
    let endpoint_path = endpoint_def.endpoint.clone();
    let handler_impl_stream = endpoint_def.handler_impl_stream.clone();
    let method_ident = format_ident!("{}", endpoint_def.method.to_string());
    let utoipa_responses = endpoint_def.utoipa_responses.clone();
    let utoipa_params = endpoint_def.utoipa_params();
    let handler = quote! {
        #[utoipa::path(#method_ident, path = #endpoint_path, #utoipa_params, #utoipa_responses, tag = #endpoint_name)]
        #[axum::debug_handler]
        pub async fn #handler_fn_name(
            extract::State(state): extract::State<RequestState>,
            #param_binding
        ) -> #handler_impl_stream
    };

    HttpEndpointMacro { 
        endpoint_def: endpoint_def.clone(),
        handler_fn_name, handler,
        client_call: endpoint_def.client_call.clone(),
        tests: endpoint_def.generate_tests(&fn_def.fn_name) 
    }
}
