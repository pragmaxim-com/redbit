use proc_macro2::{Ident, TokenStream};
use quote::quote;
use std::fmt::Display;
use syn::Type;

#[derive(Clone)]
pub struct Endpoint {
    pub handler: TokenStream,
    pub handler_fn_name: Ident,
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
    Path(Vec<PathExpr>),
    Query(QueryExpr),
    Body(BodyExpr),
}

#[derive(Clone, strum_macros::Display)]
pub enum EndpointTag {
    MetaRead,
    DataRead,
    DataWrite,
    DataDelete,
}

#[derive(Clone)]
#[allow(clippy::upper_case_acronyms)]
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
    pub fn new(fn_defs: &[FunctionDef]) -> Self {
        let endpoints: Vec<Endpoint> = fn_defs.iter().filter_map(|fn_def| fn_def.endpoint.clone()).collect();
        let endpoint_handlers: Vec<TokenStream> = endpoints.iter().map(|e| e.handler.clone()).collect();
        let handler_fn_names: Vec<Ident> = endpoints.iter().map(|e| e.handler_fn_name.clone()).collect();
        let route_array = handler_fn_names.iter().map(|name| {
            quote! { utoipa_axum::routes!(#name) }
        });

        let routes = quote! {
            pub fn routes() -> OpenApiRouter<RequestState> {
                redbit::utils::merge_route_sets([ #( #route_array ),* ])
            }
        };
        Rest {
            endpoint_handlers,
            routes,
        }
    }
}

