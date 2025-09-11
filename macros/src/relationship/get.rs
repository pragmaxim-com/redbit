use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::Path;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod, PathExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn one2one_def(entity_name: &Ident, child_name: &Ident, child_type: &Type, pk_name: &Ident, pk_type: &Type, tx_context_ty: &Type) -> FunctionDef {
    let fn_name = format_ident!("get_{}", child_name);

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = STORAGE.clone();
            let pk_value = #pk_type::default();
            let tx_context = #child_type::begin_read_tx(&storage).expect("Failed to begin read transaction context");
            let child = #entity_name::#fn_name(&tx_context, &pk_value).expect("Failed to get child by PK");
            assert_eq!(child.#pk_name, pk_value, "Child PK does not match the requested PK");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = STORAGE.clone();
            let pk_value = #pk_type::default();
            let tx_context = #child_type::begin_read_tx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, &pk_value).expect("Failed to get child by PK");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream: quote! {
            pub fn #fn_name(tx_context: &#tx_context_ty, pk: &#pk_type) -> Result<#child_type, AppError> {
                #child_type::get(&tx_context, &pk).and_then(|opt| {
                    opt.ok_or_else(|| AppError::NotFound(format!("No {} found for {:?}", stringify!(#child_name), pk)))
                })
            }
        },
        endpoint: Some(EndpointDef {
            _entity_name: entity_name.clone(),
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![Path(vec![PathExpr {
                name: pk_name.clone(),
                ty: pk_type.clone(),
                description: "Primary key".to_string(),
                sample: quote! { #pk_type::default().url_encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<#child_type>, AppError> {
                    let tx_context = #child_type::begin_read_tx(&state.storage)?;
                    let result = #entity_name::#fn_name(&tx_context, &#pk_name)?;
                    Ok(AppJson(result))
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/json", body = #child_type),
                    (status = NOT_FOUND, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{{{}}}/{}", entity_name.to_string().to_lowercase(), pk_name, child_name),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}

pub fn one2opt_def(entity_name: &Ident, child_name: &Ident, child_type: &Type, pk_name: &Ident, pk_type: &Type, tx_context_ty: &Type) -> FunctionDef {
    let fn_name = format_ident!("get_{}", child_name);

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = STORAGE.clone();
            let pk_value = #pk_type::default();
            let tx_context = #child_type::begin_read_tx(&storage).expect("Failed to begin read transaction context");
            let maybe_child = #entity_name::#fn_name(&tx_context, &pk_value).expect("Failed to get child by PK");
            assert!(maybe_child.is_none() || maybe_child.unwrap().#pk_name == pk_value, "Unexpected child PK");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = STORAGE.clone();
            let pk_value = #pk_type::default();
            let tx_context = #child_type::begin_read_tx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, &pk_value).expect("Failed to get child by PK");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream: quote! {
            pub fn #fn_name(tx_context: &#tx_context_ty, pk: &#pk_type) -> Result<Option<#child_type>, AppError> {
                #child_type::get(&tx_context, pk)
            }
        },
        endpoint: Some(EndpointDef {
            _entity_name: entity_name.clone(),
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![Path(vec![PathExpr {
                name: pk_name.clone(),
                ty: pk_type.clone(),
                description: "Primary key".to_string(),
                sample: quote! { #pk_type::default().url_encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<#child_type>, AppError> {
                 #child_type::begin_read_tx(&state.storage)
                    .and_then(|tx_context| #entity_name::#fn_name(&tx_context, &#pk_name)
                        .and_then(|opt| {
                            opt.ok_or_else(|| AppError::NotFound(format!("No {} found", stringify!(#child_name)))) }) )
                    .map(AppJson)
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/json", body = #child_type),
                    (status = NOT_FOUND, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{{{}}}/{}", entity_name.to_string().to_lowercase(), pk_name, child_name),
        }.to_endpoint()),
        test_stream,
        bench_stream,
    }
}

pub fn one2many_def(entity_name: &Ident, child_name: &Ident, child_type: &Type, pk_name: &Ident, pk_type: &Type, tx_context_ty: &Type) -> FunctionDef {
    let fn_name = format_ident!("get_{}", child_name);

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = STORAGE.clone();
            let pk_value = #pk_type::default();
            let tx_context = #child_type::begin_read_tx(&storage).expect("Failed to begin read transaction context");
            let children = #entity_name::#fn_name(&tx_context, &pk_value).expect("Failed to get children by PK");
            assert!(children.len() == 3, "Expected 3 children for the given PK");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = STORAGE.clone();
            let pk_value = #pk_type::default();
            let tx_context = #child_type::begin_read_tx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, &pk_value).expect("Failed to get children by PK");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream: quote! {
            pub fn #fn_name(tx_context: &#tx_context_ty, pk: &#pk_type) -> Result<Vec<#child_type>, AppError> {
                let (from, to) = pk.fk_range();
                #child_type::range(&tx_context, &from, &to, None)
            }
        },
        endpoint: Some(EndpointDef {
            _entity_name: entity_name.clone(),
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![Path(vec![PathExpr {
                name: pk_name.clone(),
                ty: pk_type.clone(),
                description: "Primary key".to_string(),
                sample: quote! { #pk_type::default().url_encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/json", body = Vec<#child_type>),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            handler_impl_stream: quote! {
               Result<AppJson<Vec<#child_type>>, AppError> {
                    let tx_context = #child_type::begin_read_tx(&state.storage)?;
                    let result = #entity_name::#fn_name(&tx_context, &#pk_name)?;
                    Ok(AppJson(result))
                }
            },
            endpoint: format!("/{}/{{{}}}/{}", entity_name.to_string().to_lowercase(), pk_name, child_name),
        }.to_endpoint()),
        test_stream,
        bench_stream,
    }
}
