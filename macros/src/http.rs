use proc_macro2::{Ident, TokenStream};
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
pub enum ParamExtraction {
    FromPath(Type),
    FromQuery(Type),
}

#[derive(Clone)]
pub struct EndpointDef {
    pub param_extraction: ParamExtraction,
    pub endpoint: String,
    pub method: Ident,
    pub fn_call: TokenStream,
}

fn to_route_chain(endpoints: Vec<HttpEndpointMacro>) -> Vec<TokenStream> {
    endpoints
        .into_iter()
        .map(|e| (e.endpoint_def.endpoint, e.endpoint_def.method, e.handler_fn_name))
        .map(|(endpoint, method_name, function_name)| {
            quote! {
                .route(#endpoint, ::axum::routing::#method_name(#function_name))
            }
        })
        .collect()
}

pub fn to_http_endpoints(defs: Vec<FunctionDef>) -> (Vec<HttpEndpointMacro>, Vec<TokenStream>) {
    let endpoints: Vec<HttpEndpointMacro> = defs.iter().filter_map(|fn_def| to_http_endpoint(fn_def)).collect();
    let route_chains = to_route_chain(endpoints.clone());
    (endpoints, route_chains)
}

pub fn to_http_endpoint(def: &FunctionDef) -> Option<HttpEndpointMacro> {
    if let Some(endpoint_def) = &def.endpoint_def {
        let handler_fn_name = format_ident!("{}_{}", def.entity_name.to_string().to_lowercase(), def.fn_name);
        let return_type = &def.return_type;
        let fn_call = endpoint_def.fn_call.clone();
        let param_binding = match endpoint_def.param_extraction.clone() {
            ParamExtraction::FromPath(ty) => quote! { ::axum::extract::Path(params): ::axum::extract::Path<#ty> },
            ParamExtraction::FromQuery(ty) => quote! { ::axum::extract::Query(params): ::axum::extract::Query<#ty> }
        };

        let handler = quote! {
            #[axum::debug_handler]
            pub async fn #handler_fn_name(
                ::axum::extract::State(state): ::axum::extract::State<RequestState>,
                #param_binding,
            ) -> Result<AppJson<#return_type>, AppError> {
                state.db.begin_read()
                    .map_err(AppError::from)
                    .and_then(|read_tx| #fn_call)
                    .map(AppJson)
            }
        };

        Some(HttpEndpointMacro {
            endpoint_def: endpoint_def.clone(),
            handler_fn_name,
            handler,
        })
    } else {
        None
    }
}