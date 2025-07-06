use proc_macro2::{Literal, TokenStream};
use quote::quote;
use crate::rest::{Param, HttpMethod, HttpParams};

#[derive(Clone)]
pub struct EndpointDef {
    pub params: Vec<HttpParams>,
    pub endpoint: String,
    pub method: HttpMethod,
    pub handler_impl_stream: TokenStream,
    pub utoipa_responses: TokenStream,
}

impl EndpointDef {
    pub fn generate_test(&self) -> TokenStream {
        let method_name = match self.method {
            HttpMethod::GET => quote! { http::Method::GET },
            HttpMethod::POST => quote! { http::Method::POST },
            HttpMethod::HEAD => quote! { http::Method::HEAD },
            HttpMethod::DELETE => quote! { http::Method::DELETE },
        };

        fn generate_path_expr(endpoint_path: &str, params: &[Param]) -> TokenStream {
            let mut fmt_string = endpoint_path.to_string();
            let mut fmt_args = Vec::new();

            for Param { name, ty: _, samples, .. } in params {
                let placeholder = format!("{{{}}}", name);
                if fmt_string.contains(&placeholder) {
                    fmt_string = fmt_string.replace(&placeholder, "{}");
                    fmt_args.push(samples[0].clone()); // TODO: try all samples
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

        let mut method_calls: Vec<TokenStream> = Vec::new();
        if query_param.is_some() && body_param.is_some() {
            let query_samples = query_param.unwrap().clone().samples;
            let body_param_clone = body_param.unwrap().clone();
            for query_sample in query_samples {
                for body_sample in body_param_clone.samples.iter() {
                    let ty = &body_param_clone.ty;
                    method_calls.push(quote! {
                        let query_string = serde_urlencoded::to_string(#query_sample).unwrap();
                        let final_path = format!("{}?{}", #path_expr, query_string);
                        eprintln!("Testing endpoint: {} : {} with body", #method_name, final_path);
                        let response = server.method(#method_name, &final_path).json::<#ty>(#body_sample).await;
                        response.assert_status_ok();
                    });
                }
            }
        } else if query_param.is_some() {
            let query_samples = query_param.unwrap().clone().samples;
            for query_sample in query_samples {
                method_calls.push(quote! {
                    let query_string = serde_urlencoded::to_string(#query_sample).unwrap();
                    let final_path = format!("{}?{}", #path_expr, query_string);
                    eprintln!("Testing endpoint: {} : {}", #method_name, &final_path);
                    let response = server.method(#method_name, &final_path).await;
                    response.assert_status_ok();
                });
            }
        } else if body_param.is_some() {
            let body_samples = body_param.unwrap().clone().samples;
            for body_sample in body_samples {
                let ty = body_param.unwrap().ty.clone();
                method_calls.push(quote! {
                    eprintln!("Testing endpoint: {} : {} with body", #method_name, #path_expr);
                    let response = server.method(#method_name, &#path_expr).json::<#ty>(&#body_sample).await;
                    response.assert_status_ok();
                });
            }
        } else {
            method_calls.push(quote! {
                eprintln!("Testing endpoint: {} : {}", #method_name, #path_expr);
                let response = server.method(#method_name, &#path_expr).await;
                response.assert_status_ok();
            });
        };

        quote! {
            {
                #(#method_calls)*
            }
        }
    }

    pub fn axum_bindings(&self) -> TokenStream {
        let mut bindings = vec![];

        for param in &self.params {
            match param {
                HttpParams::FromPath(path_params) => match &path_params[..] {
                    [] => {}
                    [Param { name, ty, .. }] => {
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
                HttpParams::FromQuery(param) => {
                    let ty = param.clone().ty;
                    bindings.push(quote! {
                        extract::Query(query): extract::Query<#ty>
                    });
                }
                HttpParams::FromBody(param) => {
                    let ty = param.clone().ty;
                    bindings.push(quote! {
                        AppJson(body): AppJson<#ty>
                    });
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
                HttpParams::FromBody(param) => {
                    let ct = Literal::string("application/json");
                    let ty = &param.ty;
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
