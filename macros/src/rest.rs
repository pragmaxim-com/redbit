use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;
use std::fmt::Display;
use syn::Type;

#[derive(Clone)]
pub struct Endpoint {
    pub handler: TokenStream,
    pub route: TokenStream,
    pub tests: Vec<TokenStream>,
    pub client_calls: Vec<String>,
}

#[derive(Clone)]
pub struct FunctionDef {
    pub fn_stream: TokenStream,
    pub endpoint: Option<Endpoint>,
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
pub struct QueryExpr {
    pub ty: Type,
    pub extraction: TokenStream,
    pub samples: TokenStream,
}
#[derive(Clone)]
pub struct BodyExpr {
    pub ty: Type,
    pub extraction: TokenStream,
    pub samples: TokenStream,
}

#[derive(Clone)]
pub enum HttpParams {
    FromPath(Vec<PathExpr>),
    FromQuery(QueryExpr),
    FromBody(BodyExpr),
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

pub struct Rest {
    pub endpoint_handlers: Vec<TokenStream>,
    pub routes: TokenStream,
    pub client_calls: TokenStream,
}

impl Rest {
    pub fn new(fn_defs: &Vec<FunctionDef>) -> Self {
        let endpoints: Vec<Endpoint> = fn_defs.iter().filter_map(|fn_def| fn_def.endpoint.clone()).collect();
        let route_chains: Vec<TokenStream> = endpoints.iter().map(|e| e.route.clone()).collect();
        let endpoint_handlers: Vec<TokenStream> = endpoints.iter().map(|e| e.handler.clone()).collect();
        let client_calls: Vec<String> = endpoints.into_iter().flat_map(|e| e.client_calls).collect();
        let client_calls_lit = Literal::string(&client_calls.join(""));
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
        Rest {
            endpoint_handlers,
            routes,
            client_calls
        }
    }
}

