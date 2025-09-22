use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::Path;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod, PathExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, pk_name: &Ident, pk_type: &Type, tx_context_ty: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("exists");
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#tx_context_ty, pk: &#pk_type) -> Result<bool, AppError> {
            if tx_context.#table.get(pk)?.is_some() {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let pk_value = #pk_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entity_exists = #entity_name::#fn_name(&tx_context, &pk_value).expect("Failed to check entity exists");
            assert!(entity_exists, "Entity is supposed to exist for the given PK");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let pk_value = #pk_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, &pk_value).expect("Failed to check entity exists");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: None,
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![Path(vec![PathExpr {
                name: pk_name.clone(),
                ty: pk_type.clone(),
                description: "Primary key".to_string(),
                sample: quote! { #pk_type::default().url_encode() },
            }])],
            method: HttpMethod::HEAD,
            handler_name: format_ident!("{}", handler_fn_name),
            utoipa_responses: quote! {
                responses(
                    (status = OK),
                    (status = NOT_FOUND, content_type = "application/json", body = ErrorResponse),
                )
            },
            handler_impl_stream: quote! {
                impl IntoResponse {
                    match #entity_name::begin_read_ctx(&state.storage)
                          .and_then(|tx_context| #entity_name::#fn_name(&tx_context, &#pk_name)) {
                            Ok(true) => {
                                Response::builder().status(StatusCode::OK).body(Body::empty()).unwrap().into_response()
                            },
                            Ok(false) => {
                                Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty()).unwrap().into_response()
                            },
                            Err(err) => err.into_response(),
                        }
                }
            },
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), pk_name, pk_name),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}
