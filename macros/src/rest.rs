use proc_macro2::{Ident, TokenStream};
use quote::quote;
use std::fmt::Display;
use syn::Type;

#[derive(Clone)]
pub struct Endpoint {
    pub handler: TokenStream,
    pub route: TokenStream,
    pub tests: Vec<TokenStream>,
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
    pub required: bool,
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
}

impl Rest {
    pub fn new(fn_defs: &Vec<FunctionDef>) -> Self {
        let endpoints: Vec<Endpoint> = fn_defs.iter().filter_map(|fn_def| fn_def.endpoint.clone()).collect();
        let route_chains: Vec<TokenStream> = endpoints.iter().map(|e| e.route.clone()).collect();
        let endpoint_handlers: Vec<TokenStream> = endpoints.iter().map(|e| e.handler.clone()).collect();
        let routes = quote! {
            pub fn routes() -> OpenApiRouter<RequestState> {
                OpenApiRouter::new()
                    #(#route_chains)*
            }
        };
        Rest {
            endpoint_handlers,
            routes,
        }
    }
}

