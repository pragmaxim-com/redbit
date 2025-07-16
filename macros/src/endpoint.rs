use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use crate::rest::{Endpoint, HttpMethod, HttpParams, PathExpr};

#[derive(Clone)]
pub struct EndpointDef {
    pub entity_name: Ident,
    pub fn_name: Ident,
    pub params: Vec<HttpParams>,
    pub endpoint: String,
    pub method: HttpMethod,
    pub handler_name: Ident,
    pub client_calls: Vec<String>,
    pub handler_impl_stream: TokenStream,
    pub utoipa_responses: TokenStream,
}

impl EndpointDef {
    pub fn to_endpoint(&self) -> Endpoint {
        let handler_fn_name = self.handler_name.clone();
        let endpoint_name = &self.entity_name.to_string();
        let endpoint_path = &self.endpoint.clone();
        let handler_impl_stream = &self.handler_impl_stream.clone();
        let method_ident = format_ident!("{}", &self.method.to_string());
        let utoipa_responses = &self.utoipa_responses.clone();
        let param_binding = &self.axum_bindings();
        let utoipa_params = &self.utoipa_params();
        let route = quote! { .merge(OpenApiRouter::new().routes(utoipa_axum::routes!(#handler_fn_name))) };
        let handler = quote! {
            #[utoipa::path(#method_ident, path = #endpoint_path, #utoipa_params, #utoipa_responses, tag = #endpoint_name)]
            #[axum::debug_handler]
            pub async fn #handler_fn_name(
                extract::State(state): extract::State<RequestState>,
                #param_binding
            ) -> #handler_impl_stream
        };

        Endpoint {
            handler,
            route,
            client_calls: self.client_calls.clone(),
            tests: self.generate_tests()
        }
    }

    pub fn generate_tests(&self) -> Vec<TokenStream> {
        let (server, method_name) = match self.method {
            HttpMethod::GET => (quote! { get_test_server() }, quote! { http::Method::GET }),
            HttpMethod::POST => (quote! { get_test_server() }, quote! { http::Method::POST }),
            HttpMethod::HEAD => (quote! { get_test_server() }, quote! { http::Method::HEAD }),
            HttpMethod::DELETE => (quote! { get_test_server() }, quote! { http::Method::DELETE }),
        };

        fn generate_path_expr(endpoint_path: &str, exprs: &[PathExpr]) -> TokenStream {
            let mut fmt_string: String = endpoint_path.to_string();
            let mut fmt_args: Vec<TokenStream> = Vec::new();

            for PathExpr { name, ty: _, sample, .. } in exprs {
                let placeholder = format!("{{{}}}", name);
                if fmt_string.contains(&placeholder) {
                    fmt_string = fmt_string.replace(&placeholder, "{}");
                    fmt_args.push(sample.clone());
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

        let endpoint_path = &self.endpoint;
        let mut path_expr = quote! { #endpoint_path };
        let mut query_param = None;
        let mut body_param = None;

        // Analyze and extract each param kind
        for param in &self.params {
            match param {
                HttpParams::FromPath(path_params) => {
                    path_expr = generate_path_expr(&self.endpoint, path_params);
                }
                HttpParams::FromQuery(param) => {
                    query_param = Some(param);
                }
                HttpParams::FromBody(param) => {
                    body_param = Some(param);
                }
            }
        }

        let mut tests: Vec<TokenStream> = Vec::new();
        if query_param.is_some() && body_param.is_some() {
            let query_samples = query_param.unwrap().clone().samples;
            let body_param_clone = body_param.unwrap().clone();
            let body_samples = body_param_clone.samples;
            let test_fn_name = format_ident!("http_endpoint_with_query_and_body_{}", &self.fn_name);
            tests.push(quote! {
                #[tokio::test]
                async fn #test_fn_name() {
                    for query_sample in #query_samples {
                        for body_sample in #body_samples {
                            let query_string = serde_urlencoded::to_string(query_sample.clone()).unwrap();
                            let final_path = format!("{}?{}", #path_expr, query_string);
                            eprintln!("Testing endpoint: {} : {} with body", #method_name, final_path);
                            let response =
                                match &body_sample {
                                    Some(body) => #server.await.method(#method_name, &final_path).json(&body).await,
                                    None => #server.await.method(#method_name, &final_path).await,
                                };
                            response.assert_status_ok();
                        }
                    }
                }
            });
        } else if query_param.is_some() {
            let query_samples = query_param.unwrap().clone().samples;
            let test_fn_name = format_ident!("http_endpoint_with_query_{}", &self.fn_name);
                tests.push(quote! {
                    #[tokio::test]
                    async fn #test_fn_name() {
                        for query_sample in #query_samples {
                            let query_string = serde_urlencoded::to_string(query_sample).unwrap();
                            let final_path = format!("{}?{}", #path_expr, query_string);
                            eprintln!("Testing endpoint: {} : {}", #method_name, &final_path);
                            let response = #server.await.method(#method_name, &final_path).await;
                            response.assert_status_ok();
                        }
                    }
                });
        } else if body_param.is_some() {
            let body_samples = body_param.unwrap().clone().samples;
            let test_fn_name = format_ident!("http_endpoint_with_body_{}", &self.fn_name);
            tests.push(quote! {
                #[tokio::test]
                async fn #test_fn_name() {
                    for body_sample in #body_samples {
                        eprintln!("Testing endpoint: {} : {} with body", #method_name, #path_expr);
                        let response =
                            match &body_sample {
                                Some(body) => #server.await.method(#method_name, &#path_expr).json(&body_sample).await,
                                None => #server.await.method(#method_name, &#path_expr).await,
                            };
                        response.assert_status_ok();
                    }
                }
            });
        } else {
            let test_fn_name = format_ident!("http_endpoint_{}", &self.fn_name);
            tests.push(quote! {
                #[tokio::test]
                async fn #test_fn_name() {
                    eprintln!("Testing endpoint: {} : {}", #method_name, #path_expr);
                    let response = #server.await.method(#method_name, &#path_expr).await;
                    response.assert_status_ok();
                }
            });
        };

        tests
    }

    pub fn axum_bindings(&self) -> TokenStream {
        let mut bindings = vec![];

        for param in &self.params {
            match param {
                HttpParams::FromPath(path_params) => match &path_params[..] {
                    [] => {}
                    [PathExpr { name, ty, .. }] => {
                        bindings.push(quote! { extract::Path(#name): extract::Path<#ty> });
                    }
                    _ => {
                        let names: Vec<_> = path_params.iter().map(|p| &p.name).collect();
                        let types: Vec<_> = path_params.iter().map(|p| &p.ty).collect();
                        bindings.push(quote! {
                            extract::Path((#(#names),*)): extract::Path<(#(#types),*)>
                        });
                    }
                },
                HttpParams::FromQuery(query) => {
                    bindings.push(query.extraction.clone());
                }
                HttpParams::FromBody(body) => {
                    bindings.push(body.extraction.clone());
                }
            }
        }

        quote! { #(#bindings),* }
    }

    pub fn utoipa_params(&self) -> TokenStream {
        let mut params_tokens = vec![];
        let mut body_token = None;

        for param in &self.params {
            match param {
                HttpParams::FromPath(path_params) => {
                    for p in path_params {
                        let name_str = Literal::string(&p.name.to_string());
                        let ty = &p.ty;
                        let desc = Literal::string(&p.description);
                        params_tokens.push(quote! {
                            (#name_str = #ty, Path, description = #desc)
                        });
                    }
                }
                HttpParams::FromQuery(param) => {
                    let ty = &param.ty;
                    params_tokens.push(quote! {
                        #ty
                    });
                }
                HttpParams::FromBody(body) => {
                    let ct = Literal::string("application/json");
                    let ty = &body.ty;
                    body_token = Some(quote! {
                        request_body(content = #ty, content_type = #ct)
                    });
                }
            }
        }

        let params_part = if params_tokens.is_empty() {
            quote! {}
        } else {
            quote! {
                params( #(#params_tokens),* )
            }
        };

        match body_token {
            Some(body) if !params_tokens.is_empty() => quote! {
                #params_part, #body
            },
            Some(body) => body,
            None => params_part,
        }
    }
}
