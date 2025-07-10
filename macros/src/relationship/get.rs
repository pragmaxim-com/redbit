use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::FromPath;
use crate::rest::{FunctionDef, HttpMethod, PathExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn one2one_def(entity_name: &Ident, child_name: &Ident, child_type: &Type, pk_name: &Ident, pk_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("get_{}", child_name);

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let pk_value = #pk_type::default();
            let child = #entity_name::#fn_name(&read_tx, &pk_value).expect("Failed to get child by PK");
            assert_eq!(child.#pk_name, pk_value, "Child PK does not match the requested PK");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let pk_value = #pk_type::default();
            b.iter(|| {
                #entity_name::#fn_name(&read_tx, &pk_value).expect("Failed to get child by PK");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream: quote! {
            pub fn #fn_name(tx: &ReadTransaction, pk: &#pk_type) -> Result<#child_type, AppError> {
                #child_type::get(&tx, &pk).and_then(|opt| {
                    opt.ok_or_else(|| AppError::Internal(format!("No child found for pk: {:?}", pk)))
                })
            }
        },
        endpoint_def: Some(EndpointDef {
            params: vec![FromPath(vec![PathExpr {
                name: pk_name.clone(),
                ty: pk_type.clone(),
                description: "Primary key".to_string(),
                sample: quote! { #pk_type::default().encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<#child_type>, AppError> {
                    state.db.begin_read().map_err(AppError::from).and_then(|tx| #entity_name::#fn_name(&tx, &#pk_name)).map(AppJson)
                }
            },
            utoipa_responses: quote! { responses((status = OK, body = #child_type)) },
            endpoint: format!("/{}/{{{}}}/{}", entity_name.to_string().to_lowercase(), pk_name, child_name),
        }),
        test_stream,
        bench_stream
    }
}

pub fn one2opt_def(entity_name: &Ident, child_name: &Ident, child_type: &Type, pk_name: &Ident, pk_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("get_{}", child_name);

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let pk_value = #pk_type::default();
            let maybe_child = #entity_name::#fn_name(&read_tx, &pk_value).expect("Failed to get child by PK");
            assert!(maybe_child.is_none() || maybe_child.unwrap().#pk_name == pk_value, "Unexpected child PK");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let pk_value = #pk_type::default();
            b.iter(|| {
                #entity_name::#fn_name(&read_tx, &pk_value).expect("Failed to get child by PK");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream: quote! {
            pub fn #fn_name(
                tx: &ReadTransaction,
                pk: &#pk_type
            ) -> Result<Option<#child_type>, AppError> {
                #child_type::get(&tx, &pk)
            }
        },
        endpoint_def: Some(EndpointDef {
            params: vec![FromPath(vec![PathExpr {
                name: pk_name.clone(),
                ty: pk_type.clone(),
                description: "Primary key".to_string(),
                sample: quote! { #pk_type::default().encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<Option<#child_type>>, AppError> {
                    state.db.begin_read().map_err(AppError::from).and_then(|tx| #entity_name::#fn_name(&tx, &#pk_name)).map(AppJson)
                }
            },
            utoipa_responses: quote! { responses((status = OK, body = Option<#child_type>)) },
            endpoint: format!("/{}/{{{}}}/{}", entity_name.to_string().to_lowercase(), pk_name, child_name),
        }),
        test_stream,
        bench_stream,
    }
}

pub fn one2many_def(entity_name: &Ident, child_name: &Ident, child_type: &Type, pk_name: &Ident, pk_type: &Type) -> FunctionDef {
    let fn_name = format_ident!("get_{}", child_name);

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let pk_value = #pk_type::default();
            let children = #entity_name::#fn_name(&read_tx, &pk_value).expect("Failed to get children by PK");
            assert!(children.len() == 3, "Expected 3 children for the given PK");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let pk_value = #pk_type::default();
            b.iter(|| {
                #entity_name::#fn_name(&read_tx, &pk_value).expect("Failed to get children by PK");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        entity_name: entity_name.clone(),
        fn_name: fn_name.clone(),
        fn_stream: quote! {
            pub fn #fn_name(tx: &ReadTransaction, pk: &#pk_type) -> Result<Vec<#child_type>, AppError> {
                let (from, to) = pk.fk_range();
                #child_type::range(&tx, &from, &to, None)
            }
        },
        endpoint_def: Some(EndpointDef {
            params: vec![FromPath(vec![PathExpr {
                name: pk_name.clone(),
                ty: pk_type.clone(),
                description: "Primary key".to_string(),
                sample: quote! { #pk_type::default().encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            utoipa_responses: quote! { responses((status = OK, body = Vec<#child_type>)) },
            handler_impl_stream: quote! {
               Result<AppJson<Vec<#child_type>>, AppError> {
                    state.db.begin_read().map_err(AppError::from).and_then(|tx| #entity_name::#fn_name(&tx, &#pk_name)).map(AppJson)
                }
            },
            endpoint: format!("/{}/{{{}}}/{}", entity_name.to_string().to_lowercase(), pk_name, child_name),
        }),
        test_stream,
        bench_stream,
    }
}
